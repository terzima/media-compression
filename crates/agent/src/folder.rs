//! Deterministic local folder workflow shared by CLI and MCP.
use crate::session::Session;
use anyhow::{bail, Context, Result};
use media_engine::{hash_file_cancelable, Candidate, Media, Properties, Settings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Lossless,
    Smaller,
    Both,
}
impl Mode {
    fn policies(self) -> &'static [Mode] {
        match self {
            Self::Lossless => &[Self::Lossless],
            Self::Smaller => &[Self::Smaller],
            Self::Both => &[Self::Lossless, Self::Smaller],
        }
    }
    fn folder(self) -> &'static str {
        match self {
            Self::Lossless => "Lossless",
            Self::Smaller => "Smaller",
            Self::Both => unreachable!(),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderRequest {
    pub path: PathBuf,
    #[serde(default)]
    pub mode: Mode,
    pub destination: Option<PathBuf>,
}

struct Active<'a>(&'a Session);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.folder_active.store(false, Ordering::SeqCst);
    }
}

fn setting(p: &Properties, mode: Mode) -> Option<Settings> {
    let mut s = Settings {
        format: String::new(),
        lossless: mode == Mode::Lossless,
        quality: None,
        bitrate: None,
        vbr_quality: None,
        effort: None,
        background: None,
    };
    if p.kind == "image" {
        if mode == Mode::Lossless {
            s.format =
                if p.bit_depth.unwrap_or(8) > 8 || p.color_profile.as_deref() == Some("gray") {
                    "png"
                } else {
                    "webp"
                }
                .into();
            s.effort = Some(if s.format == "png" { 2 } else { 4 });
        } else {
            s.format = "webp".into();
            s.quality = Some(90.);
            s.effort = Some(4);
        }
    } else if p.kind == "audio" {
        if mode == Mode::Lossless {
            s.format = "flac".into();
            s.effort = Some(5);
        } else {
            s.format = "aac".into();
            s.bitrate = Some(if p.channels == Some(1) { 96. } else { 192. });
            // An automatic preset must not silently resample. Studies disclose conversions.
            if p.sample_rate != Some(media_engine::settings::output_rate(&s, p)) {
                return None;
            }
        }
    } else {
        return None;
    }
    media_engine::settings::validate(&s, p).ok().map(|_| s)
}

fn verified(c: &Candidate, p: &Properties, mode: Mode) -> bool {
    if c.properties.kind != p.kind {
        return false;
    }
    if p.kind == "image" {
        c.properties.width == p.width
            && c.properties.height == p.height
            && (!p.alpha || c.diagnostics.alpha_max_error == Some(0))
            && (mode != Mode::Lossless
                || (c.diagnostics.pixel_identical == Some(true)
                    && c.properties.color_profile == p.color_profile
                    && (p.bit_depth != Some(16) || c.properties.bit_depth == Some(16))))
    } else {
        c.properties.channels == p.channels
            && c.properties.channel_layout == p.channel_layout
            && c.properties.sample_rate == p.sample_rate
            && (mode != Mode::Lossless
                || (c.diagnostics.samples_identical == Some(true)
                    && c.properties.bit_depth == p.bit_depth))
    }
}

fn is_redirect(metadata: &fs::Metadata) -> bool {
    let link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        link || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        link
    }
}

fn collect(
    session: &Session,
    dir: &Path,
    exclusions: &[PathBuf],
    files: &mut Vec<PathBuf>,
    skipped: &mut Vec<Value>,
    errors: &mut Vec<Value>,
) -> Result<()> {
    if session.folder_canceled.load(Ordering::SeqCst) {
        bail!("Canceled");
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(json!({"path":dir,"error":format!("Cannot read folder: {e}")}));
            return Ok(());
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(e) => paths.push(e.path()),
            Err(e) => errors.push(json!({"path":dir,"error":format!("Cannot read entry: {e}")})),
        }
    }
    paths.sort();
    for path in paths {
        if session.folder_canceled.load(Ordering::SeqCst) {
            bail!("Canceled");
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(json!({"path":path,"error":e.to_string()}));
                continue;
            }
        };
        if is_redirect(&metadata) {
            skipped.push(json!({"path":path,"reason":"Link/junction skipped"}));
            continue;
        }
        if metadata.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.eq_ignore_ascii_case("Lossless")
                || name.eq_ignore_ascii_case("Smaller")
                || exclusions.iter().any(|p| path.starts_with(p))
            {
                skipped
                    .push(json!({"path":path,"reason":"Generated output/cache folder excluded"}));
                continue;
            }
            session.scope.path(&path)?;
            collect(session, &path, exclusions, files, skipped, errors)?;
        } else if metadata.is_file() {
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if [
                "png", "jpg", "jpeg", "webp", "wav", "flac", "mp3", "aac", "m4a", "ogg", "opus",
            ]
            .contains(&extension.as_str())
            {
                files.push(session.scope.path(&path)?);
                if files.len() > 10000 {
                    bail!(
                        "Folder contains more than 10,000 supported files; select a smaller folder"
                    );
                }
            } else {
                skipped.push(json!({"path":path,"reason":"Unsupported file extension"}));
            }
        }
    }
    Ok(())
}

impl Session {
    /// Waits internally; callers need no per-file plans, polling scripts or candidate selection.
    pub fn compress_folder(&self, request: FolderRequest) -> Result<Value> {
        let _operation = self.operation()?;
        if self.active() {
            bail!("Finish or cancel the active study before compressing a folder");
        }
        self.folder_canceled.store(false, Ordering::SeqCst);
        self.folder_active.store(true, Ordering::SeqCst);
        let _active = Active(self);
        let started = Instant::now();
        let root = self.scope.path(&request.path)?;
        if !root.is_dir() {
            bail!("compress-folder requires an existing folder");
        }
        let destination = self
            .scope
            .path(request.destination.as_deref().unwrap_or(&root))?;
        if destination.exists() && !destination.is_dir() {
            bail!("Destination must be a folder");
        }
        let mut folders = Vec::new();
        for policy in request.mode.policies() {
            let output = destination.join(policy.folder());
            if fs::symlink_metadata(&output).is_ok_and(|m| is_redirect(&m)) {
                bail!("Output folder must not be a link/junction");
            }
            folders.push((
                *policy,
                self.scope.path(&destination.join(policy.folder()))?,
            ));
        }
        let mut exclusions = vec![self.workspace.canonicalize()?];
        if destination != root {
            exclusions.push(destination.clone());
        }
        let mut files = Vec::new();
        let mut skipped = Vec::new();
        let mut errors = Vec::new();
        collect(
            self,
            &root,
            &exclusions,
            &mut files,
            &mut skipped,
            &mut errors,
        )?;
        let old: BTreeMap<_, _> = self
            .engine
            .snapshot()
            .media
            .into_iter()
            .map(|m| (m.path.clone(), m))
            .collect();
        let mut imports = Vec::new();
        for path in &files {
            if self.folder_canceled.load(Ordering::SeqCst) {
                break;
            }
            let unchanged = old
                .get(path)
                .filter(|m| m.error.is_none())
                .is_some_and(|m| {
                    hash_file_cancelable(path, &self.folder_canceled)
                        .ok()
                        .as_ref()
                        == Some(&m.sha256)
                });
            if !unchanged {
                imports.push(path.clone());
            }
        }
        self.engine
            .import_scoped_cancelable(imports, &self.scope.roots, &self.folder_canceled)?;
        let current: BTreeMap<_, _> = self
            .engine
            .snapshot()
            .media
            .into_iter()
            .map(|m| (m.path.clone(), m))
            .collect();
        let mut media = Vec::<Media>::new();
        for path in files {
            if self.folder_canceled.load(Ordering::SeqCst) {
                break;
            }
            let m = current.get(&path).context("Imported file missing")?.clone();
            if let Some(e) = &m.error {
                errors.push(json!({"path":path,"error":e}));
            } else {
                self.engine
                    .restore_import_name(&m.id, &path.strip_prefix(&root)?.to_string_lossy())?;
            }
            media.push(m);
        }
        let mut items = Vec::new();
        for m in &media {
            if let Some(p) = m.properties.as_ref().filter(|_| m.error.is_none()) {
                let settings: Vec<_> = request
                    .mode
                    .policies()
                    .iter()
                    .filter_map(|mode| setting(p, *mode))
                    .collect();
                if !settings.is_empty() {
                    items.push((m.id.clone(), settings));
                }
            }
        }
        let planned: usize = items.iter().map(|(_, s)| s.len()).sum();
        let jobs = if !items.is_empty() && !self.folder_canceled.load(Ordering::SeqCst) {
            self.engine.start(items)?
        } else {
            Vec::new()
        };
        loop {
            if self.folder_canceled.load(Ordering::SeqCst) {
                self.engine.cancel(None);
            }
            let data = self.engine.snapshot();
            if !data.jobs.iter().any(|j| {
                jobs.contains(&j.id) && ["queued", "processing"].contains(&j.state.as_str())
            }) {
                break;
            }
            std::thread::sleep(Duration::from_millis(80));
        }
        let data = self.engine.snapshot();
        for j in &data.jobs {
            if jobs.contains(&j.id) {
                for e in &j.errors {
                    errors.push(json!({"mediaId":j.media_id,"error":e}));
                }
            }
        }
        let mut rows = Vec::new();
        let mut summaries = Vec::new();
        for (policy, folder) in folders {
            let mut original_bytes = 0u64;
            let mut output_bytes = 0u64;
            let mut exported = 0usize;
            let mut encoded = 0usize;
            for source in &media {
                if self.folder_canceled.load(Ordering::SeqCst) {
                    break;
                }
                let Some(p) = source
                    .properties
                    .as_ref()
                    .filter(|_| source.error.is_none())
                else {
                    continue;
                };
                let expected = setting(p, policy);
                let candidate = expected.as_ref().and_then(|s| {
                    data.media
                        .iter()
                        .find(|m| m.id == source.id)
                        .and_then(|m| m.candidates.iter().find(|c| c.settings.contains(s)))
                });
                let keep_reason = if expected.is_none() {
                    Some("Preset cannot preserve this source's supported precision/profile/sample rate; copied the original")
                } else if let Some(c) = candidate.filter(|c| verified(c, p, policy)) {
                    if c.bytes >= source.bytes {
                        Some("Candidate is not smaller; copied the original")
                    } else {
                        None
                    }
                } else {
                    errors.push(json!({"path":source.path,"mode":policy,"error":"No verified candidate for this preset"}));
                    continue;
                };
                let result = if keep_reason.is_some() {
                    self.engine.export_original_cancelable(
                        &source.id,
                        &folder,
                        &self.folder_canceled,
                    )
                } else {
                    let c = candidate.unwrap();
                    self.engine
                        .export_cancelable(
                            std::slice::from_ref(&c.id),
                            &folder,
                            false,
                            &self.folder_canceled,
                        )
                        .and_then(|paths| {
                            Ok(folder.join(paths.first().context("Export produced no file")?))
                        })
                };
                match result {
                    Ok(path) => {
                        let c = candidate.filter(|_| keep_reason.is_none());
                        let bytes = c.map_or(source.bytes, |c| c.bytes);
                        original_bytes += source.bytes;
                        output_bytes += bytes;
                        exported += 1;
                        encoded += usize::from(c.is_some());
                        rows.push(json!({"source":source.path,"mode":policy,"output":path,"disposition":if c.is_some(){"compressed"}else{"kept_original"},
                            "originalBytes":source.bytes,"outputBytes":bytes,"bytesSaved":source.bytes-bytes,
                            "sourceSha256":source.sha256,"sha256":c.map_or(&source.sha256,|c|&c.sha256),"settings":c.and(expected),"diagnostics":c.map(|c|&c.diagnostics),"notice":keep_reason}));
                    }
                    Err(e) => errors
                        .push(json!({"path":source.path,"mode":policy,"error":format!("{e:#}")})),
                }
            }
            summaries.push(json!({"mode":policy,"folder":folder,"exportedFiles":exported,"compressedFiles":encoded,"keptOriginalFiles":exported-encoded,
                "originalBytes":original_bytes,"outputBytes":output_bytes,"bytesSaved":original_bytes-output_bytes,
                "savingsPercent":if original_bytes==0{None}else{Some((1.-output_bytes as f64/original_bytes as f64)*100.)}}));
        }
        Ok(
            json!({"schemaVersion":1,"applicationVersion":env!("CARGO_PKG_VERSION"),"path":root,"destination":destination,"mode":request.mode,
            "sourceFiles":media.len(),"plannedEncodes":planned,"elapsedSeconds":started.elapsed().as_secs_f64(),"canceled":self.folder_canceled.load(Ordering::SeqCst),
            "summaries":summaries,"outputs":rows,"skipped":skipped,"errors":errors,
            "notice":"Deterministic preset, not a search for an optimum. Images: WebP lossless effort 4 or lossy quality 90 effort 4; precision/profile fallback PNG effort 2. Audio: FLAC level 5 or AAC-LC 192 kbps stereo/96 mono. Larger results keep the original. Lossy quality is not guaranteed invisible. Verified cache is reused; originals stay unchanged."}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_presets_keep_precision_and_never_silently_resample() {
        let mut p = Properties {
            kind: "image".into(),
            format: "png".into(),
            bit_depth: Some(16),
            ..Default::default()
        };
        assert_eq!(setting(&p, Mode::Lossless).unwrap().format, "png");
        assert!(setting(&p, Mode::Smaller).is_none());
        p = Properties {
            kind: "audio".into(),
            format: "wav".into(),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_depth: Some(24),
            sample_format: Some("s32".into()),
            ..Default::default()
        };
        assert_eq!(setting(&p, Mode::Lossless).unwrap().format, "flac");
        assert_eq!(setting(&p, Mode::Smaller).unwrap().bitrate, Some(192.));
        p.sample_rate = Some(12345);
        assert!(setting(&p, Mode::Smaller).is_none());
        p.sample_format = Some("flt".into());
        assert!(setting(&p, Mode::Lossless).is_none());
    }
    #[test]
    fn folder_request_defaults_safe_and_rejects_unknown_policy_fields() {
        let request: FolderRequest = serde_json::from_value(json!({"path":"/media"})).unwrap();
        assert_eq!(request.mode, Mode::Lossless);
        assert!(serde_json::from_value::<FolderRequest>(
            json!({"path":"/media","mode":"invisible"})
        )
        .is_err());
        assert!(
            serde_json::from_value::<FolderRequest>(json!({"path":"/media","overwite":true}))
                .is_err()
        );
    }
}

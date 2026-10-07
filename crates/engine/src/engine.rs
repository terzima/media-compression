use crate::{process, settings, *};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use uuid::Uuid;

pub struct Engine {
    pub root: PathBuf,
    pub tools: Tools,
    data: Mutex<Snapshot>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    busy: AtomicBool,
    image_gate: Mutex<()>,
    notify: Box<dyn Fn() + Send + Sync>,
    tool_identity: String,
    _lock: File,
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}
fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}
pub fn extension(format: &str) -> &str {
    match format {
        "aac" => "m4a",
        "jpeg" => "jpg",
        x => x,
    }
}

impl Engine {
    pub fn new(
        root: PathBuf,
        tools: Tools,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Result<Arc<Self>> {
        fs::create_dir_all(&root)?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("session.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .context("Another instance is using this workspace")?;
        let work = root.join("work");
        if work.exists() {
            fs::remove_dir_all(&work)?;
        }
        fs::create_dir_all(&work)?;
        fs::create_dir_all(root.join("cache"))?;
        let mut versions = Vec::new();
        let mut identity = Sha256::new();
        for (name, path, args) in [
            ("ffmpeg", &tools.ffmpeg, vec!["-version"]),
            ("ffprobe", &tools.ffprobe, vec!["-version"]),
            ("pngquant", &tools.pngquant, vec!["--version"]),
            ("cwebp", &tools.cwebp, vec!["-version"]),
            ("cjpeg", &tools.cjpeg, vec!["-version"]),
        ] {
            let sha = hash_file(path)
                .with_context(|| format!("Bundled {name} is missing; rebuild the codec package"))?;
            identity.update(sha.as_bytes());
            let output = process::run(
                path,
                &strings(&args),
                None,
                &AtomicBool::new(false),
                Some(Duration::from_secs(10)),
            )?;
            versions.push(format!(
                "{name}: {}",
                String::from_utf8_lossy(&output)
                    .lines()
                    .next()
                    .unwrap_or("bundled")
            ));
        }
        identity.update(hash_file(&tools.worker)?.as_bytes());
        Ok(Arc::new(Self {
            root,
            tools,
            data: Mutex::new(Snapshot {
                tools: versions,
                ..Default::default()
            }),
            cancels: Mutex::new(HashMap::new()),
            busy: AtomicBool::new(false),
            image_gate: Mutex::new(()),
            notify: Box::new(notify),
            tool_identity: hex::encode(identity.finalize()),
            _lock: lock,
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.data.lock().unwrap().clone()
    }
    fn changed(&self) {
        (self.notify)();
    }
    pub fn import(&self, paths: Vec<PathBuf>) -> Result<()> {
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            if path.is_dir() && !fs::symlink_metadata(&path)?.file_type().is_symlink() {
                let prefix = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                collect_files(&path, &path, &prefix, &mut files)?;
            } else {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                files.push((path, name));
            }
        }
        for (path, relative) in files {
            if files_too_many(self.data.lock().unwrap().media.len()) {
                bail!(
                    "The batch limit is 10,000 files; import another batch after clearing this one"
                );
            }
            let id = Uuid::new_v4().to_string();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let mut item = Media {
                id: id.clone(),
                name,
                relative_name: relative,
                bytes: 0,
                sha256: String::new(),
                properties: None,
                preview: None,
                error: None,
                candidates: Vec::new(),
                path: path.clone(),
            };
            let result = (|| -> Result<()> {
                if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    bail!("Symbolic links are skipped to prevent cycles or accidental traversal");
                }
                item.path = path.canonicalize()?;
                if !seen.insert(item.path.clone()) {
                    bail!("Duplicate import in this selection");
                }
                if self
                    .data
                    .lock()
                    .unwrap()
                    .media
                    .iter()
                    .any(|m| m.path == item.path)
                {
                    bail!("This file is already imported");
                }
                item.bytes = fs::metadata(&item.path)?.len();
                if item.bytes == 0 {
                    bail!("Empty file");
                }
                item.sha256 = hash_file(&item.path)?;
                let probe = probe_audio(&self.tools, &item.path, &AtomicBool::new(false))?;
                if probe.kind == "image" {
                    let preview = self.root.join("work").join(format!("{id}-source.png"));
                    let result = self.image_request(
                        "inspect",
                        &item.path,
                        &preview,
                        None,
                        &AtomicBool::new(false),
                    )?;
                    item.properties = Some(result.properties);
                } else {
                    item.properties = Some(probe);
                }
                if hash_file(&item.path)? != item.sha256 {
                    bail!("Source changed during import; add it again");
                }
                Ok(())
            })();
            if let Err(e) = result {
                item.error = Some(format!("{e:#}"));
            }
            self.data.lock().unwrap().media.push(item);
            self.changed();
        }
        Ok(())
    }
    fn image_request(
        &self,
        action: &str,
        input: &Path,
        output: &Path,
        settings: Option<Settings>,
        cancel: &AtomicBool,
    ) -> Result<ImageResult> {
        let _gate = self.image_gate.lock().unwrap();
        let request = ImageRequest {
            action: action.into(),
            input: input.into(),
            output: output.into(),
            settings,
            tools: self.tools.clone(),
        };
        let bytes = process::run(
            &self.tools.worker,
            &[],
            Some(&serde_json::to_vec(&request)?),
            cancel,
            if action != "compress" {
                Some(Duration::from_secs(60))
            } else {
                None
            },
        )?;
        serde_json::from_slice(&bytes).context("Image worker returned invalid data")
    }
    pub fn start(self: &Arc<Self>, items: Vec<(String, Vec<Settings>)>) -> Result<Vec<String>> {
        if self.busy.swap(true, Ordering::SeqCst) {
            bail!("Wait for the current batch or cancel it before starting another");
        }
        let result = (|| -> Result<_> {
            let mut tasks = VecDeque::new();
            let mut ids = Vec::new();
            let mut data = self.data.lock().unwrap();
            let mut cancels = self.cancels.lock().unwrap();
            // Validate the whole request before mutating jobs or starting processes.
            for (media_id, options) in &items {
                if options.is_empty() || options.len() > 512 {
                    bail!("Provide 1–512 settings per study");
                }
                let media = data
                    .media
                    .iter()
                    .find(|m| &m.id == media_id)
                    .context("Imported file no longer exists")?;
                if media.error.is_none() {
                    let properties = media.properties.as_ref().context("Missing properties")?;
                    for setting in options {
                        settings::validate(setting, properties)?;
                    }
                }
            }
            for (media_id, options) in items {
                if options.is_empty() || options.len() > 512 {
                    bail!("Provide 1–512 settings per study");
                }
                let media = data
                    .media
                    .iter()
                    .find(|m| m.id == media_id)
                    .context("Imported file no longer exists")?
                    .clone();
                if media.error.is_some() {
                    continue;
                }
                let id = Uuid::new_v4().to_string();
                let cancel = Arc::new(AtomicBool::new(false));
                data.jobs.push(Job {
                    id: id.clone(),
                    media_id,
                    state: "queued".into(),
                    stage: "Waiting".into(),
                    completed: 0,
                    total: options.len(),
                    errors: Vec::new(),
                });
                cancels.insert(id.clone(), cancel.clone());
                ids.push(id.clone());
                tasks.push_back((id, media, options, cancel));
            }
            Ok((tasks, ids))
        })();
        let (tasks, ids) = match result {
            Ok(x) => x,
            Err(e) => {
                self.busy.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };
        if tasks.is_empty() {
            self.busy.store(false, Ordering::SeqCst);
            return Ok(ids);
        }
        let tasks = Arc::new(Mutex::new(tasks));
        let remaining = Arc::new(AtomicUsize::new(2));
        for _ in 0..2 {
            let engine = self.clone();
            let tasks = tasks.clone();
            let remaining = remaining.clone();
            std::thread::spawn(move || {
                loop {
                    let task = tasks.lock().unwrap().pop_front();
                    let Some((id, media, options, cancel)) = task else {
                        break;
                    };
                    engine.run_job(&id, &media, options, &cancel);
                    engine.cancels.lock().unwrap().remove(&id);
                }
                if remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
                    engine.busy.store(false, Ordering::SeqCst);
                    engine.changed();
                }
            });
        }
        self.changed();
        Ok(ids)
    }
    fn update_job(&self, id: &str, update: impl FnOnce(&mut Job)) {
        if let Some(j) = self
            .data
            .lock()
            .unwrap()
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
        {
            update(j);
        }
        self.changed();
    }
    fn run_job(&self, id: &str, media: &Media, options: Vec<Settings>, cancel: &AtomicBool) {
        if cancel.load(Ordering::Relaxed) {
            self.update_job(id, |j| {
                j.state = "canceled".into();
                j.stage = "Canceled before encoding".into();
            });
            return;
        }
        self.update_job(id, |j| {
            j.state = "processing".into();
            j.stage = "Preparing".into();
        });
        let mut successes = 0;
        for (index, s) in options.into_iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            self.update_job(id, |j| {
                j.stage = format!("Encoding candidate {} of {}", index + 1, j.total)
            });
            match self.encode(media, s, cancel) {
                Ok(candidate) => {
                    successes += 1;
                    let mut data = self.data.lock().unwrap();
                    if let Some(m) = data.media.iter_mut().find(|m| m.id == media.id) {
                        if let Some(existing) = m
                            .candidates
                            .iter_mut()
                            .find(|c| c.sha256 == candidate.sha256)
                        {
                            for s in candidate.settings {
                                if !existing.settings.iter().any(|v| {
                                    serde_json::to_string(v).ok() == serde_json::to_string(&s).ok()
                                }) {
                                    existing.settings.push(s);
                                }
                            }
                        } else {
                            m.candidates.push(candidate);
                        }
                    }
                }
                Err(error) => {
                    if !cancel.load(Ordering::Relaxed) {
                        self.update_job(id, |j| {
                            j.errors.push(format!("Candidate {}: {error:#}", index + 1))
                        });
                    }
                }
            }
            self.update_job(id, |j| j.completed = index + 1);
        }
        self.update_job(id, |j| {
            j.state = if cancel.load(Ordering::Relaxed) {
                "canceled"
            } else if successes > 0 {
                "ready"
            } else {
                "failed"
            }
            .into();
            j.stage = if j.state == "ready" {
                "Finished; choose candidates to export"
            } else if j.state == "canceled" {
                "Canceled; completed candidates retained"
            } else {
                "No valid candidates"
            }
            .into();
        });
    }
    fn encode(&self, media: &Media, s: Settings, cancel: &AtomicBool) -> Result<Candidate> {
        let p = media.properties.as_ref().context("File is not supported")?;
        settings::validate(&s, p)?;
        if hash_file(&media.path)? != media.sha256 {
            bail!("Source changed; import the file again");
        }
        let mut hash = Sha256::new();
        hash.update(media.sha256.as_bytes());
        hash.update(serde_json::to_vec(&s)?);
        hash.update(self.tool_identity.as_bytes());
        let key = hex::encode(hash.finalize());
        let output = self
            .root
            .join("cache")
            .join(format!("{key}.{}", extension(&s.format)));
        let record = output.with_extension("json");
        let mut candidate = if record.exists() && output.exists() {
            let c: Candidate = match serde_json::from_slice(&fs::read(&record)?) {
                Ok(c) => c,
                Err(_) => {
                    fs::remove_file(&record)?;
                    fs::remove_file(&output)?;
                    return self.encode(media, s, cancel);
                }
            };
            if fs::metadata(&output)?.len() == c.bytes && hash_file(&output)? == c.sha256 {
                c
            } else {
                fs::remove_file(&record)?;
                fs::remove_file(&output)?;
                return self.encode(media, s, cancel);
            }
        } else {
            if fs2::available_space(&self.root)?
                < media.bytes.saturating_mul(2).max(16 * 1024 * 1024)
            {
                bail!("Not enough temporary disk space");
            }
            let temp = tempfile::Builder::new()
                .prefix("candidate-")
                .tempdir_in(self.root.join("work"))?;
            let stage = temp.path().join(format!("output.{}", extension(&s.format)));
            let result = if p.kind == "image" {
                self.image_request("compress", &media.path, &stage, Some(s.clone()), cancel)?
            } else {
                self.encode_audio(media, &s, &stage, cancel)?
            };
            if cancel.load(Ordering::Relaxed) {
                bail!("Canceled");
            }
            if hash_file(&media.path)? != media.sha256 {
                bail!("Source changed while processing");
            }
            let bytes = fs::metadata(&stage)?.len();
            if bytes == 0 {
                bail!("Encoder returned an empty file");
            }
            let sha256 = hash_file(&stage)?;
            let c = Candidate {
                id: Uuid::new_v4().to_string(),
                media_id: media.id.clone(),
                bytes,
                sha256,
                settings: vec![s.clone()],
                properties: result.properties,
                diagnostics: result.diagnostics,
                preview: None,
                exported: Vec::new(),
                path: output.clone(),
            };
            // Same-filesystem staging; cache keys never overwrite an unverified candidate.
            let mut cache_stage = tempfile::NamedTempFile::new_in(self.root.join("cache"))?;
            std::io::copy(&mut File::open(&stage)?, &mut cache_stage)?;
            cache_stage.as_file().sync_all()?;
            match cache_stage.persist_noclobber(&output) {
                Ok(_) => {}
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if hash_file(&output)? != c.sha256 {
                        bail!("Cache identity collision");
                    }
                }
                Err(e) => return Err(e.error.into()),
            }
            let mut metadata = tempfile::NamedTempFile::new_in(self.root.join("cache"))?;
            metadata.write_all(&serde_json::to_vec(&c)?)?;
            metadata.as_file().sync_all()?;
            metadata.persist(&record)?;
            c
        };
        candidate.media_id = media.id.clone();
        candidate.id = Uuid::new_v4().to_string();
        candidate.path = output.clone();
        candidate.exported.clear();
        Ok(candidate)
    }
    fn encode_audio(
        &self,
        media: &Media,
        s: &Settings,
        output: &Path,
        cancel: &AtomicBool,
    ) -> Result<ImageResult> {
        let p = media.properties.as_ref().context("Missing properties")?;
        let rate = settings::output_rate(s, p);
        let mut args = strings(&[
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-threads",
            "1",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]);
        args.push(media.path.to_string_lossy().into());
        args.extend(strings(&[
            "-map",
            "0:a:0",
            "-vn",
            "-sn",
            "-dn",
            "-map_metadata",
            "-1",
        ]));
        let probe = self.probe_json(&media.path, cancel)?;
        for name in [
            "title", "artist", "album", "date", "track", "genre", "comment",
        ] {
            // Only ordinary text tags are copied; no arbitrary cover, chapters or container metadata.
            if let Some(value) = probe["format"]["tags"]
                .as_object()
                .and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)))
                .and_then(|(_, v)| v.as_str())
            {
                args.extend(["-metadata".into(), format!("{name}={value}")]);
            }
        }
        args.extend([
            "-ar".into(),
            rate.to_string(),
            "-ac".into(),
            p.channels.unwrap_or(2).to_string(),
        ]);
        match s.format.as_str() {
            "aac" => {
                args.extend(strings(&["-c:a", "aac", "-profile:a", "aac_low"]));
                args.extend(["-b:a".into(), format!("{}k", s.bitrate.unwrap())]);
            }
            "mp3" => {
                args.extend(strings(&["-c:a", "libmp3lame"]));
                if let Some(q) = s.vbr_quality {
                    args.extend(["-q:a".into(), q.to_string()]);
                } else {
                    args.extend(["-b:a".into(), format!("{}k", s.bitrate.unwrap())]);
                }
            }
            "opus" => {
                args.extend(strings(&["-c:a", "libopus", "-vbr", "on"]));
                args.extend(["-b:a".into(), format!("{}k", s.bitrate.unwrap())]);
            }
            "flac" => {
                args.extend(strings(&["-c:a", "flac"]));
                args.extend([
                    "-compression_level".into(),
                    s.effort.unwrap_or(5).to_string(),
                ]);
            }
            _ => bail!("Unsupported encoder"),
        }
        if ["mp3", "opus"].contains(&s.format.as_str()) {
            if let Some(effort) = s.effort {
                args.extend(["-compression_level".into(), effort.to_string()]);
            }
        }
        args.extend([
            "-threads".into(),
            "1".into(),
            "-n".into(),
            output.to_string_lossy().into(),
        ]);
        process::run(&self.tools.ffmpeg, &args, None, cancel, None)?;
        let after = probe_audio(&self.tools, output, cancel)?;
        if after.channels != p.channels || after.sample_rate != Some(rate) {
            bail!("Encoder changed channels or sample rate unexpectedly");
        }
        let before_frames = self.decoded_frames(&media.path, rate, cancel)?;
        let after_frames = self.decoded_frames(output, rate, cancel)?;
        let delta = after_frames as i64 - before_frames as i64;
        let tolerance = match s.format.as_str() {
            "aac" => 1024,
            "mp3" => 1152,
            "opus" => 960,
            _ => 0,
        };
        if delta.abs() > tolerance || (before_frames > 0 && after_frames == 0) {
            bail!("Unexpected decoded timeline change: {delta} frames");
        }
        if s.format == "flac"
            && self.pcm_hash(&media.path, rate, cancel)? != self.pcm_hash(output, rate, cancel)?
        {
            bail!("FLAC decoded samples differ from source");
        }
        let mut notices =
            vec!["Basic text tags preserved; artwork and other metadata omitted".into()];
        if rate != p.sample_rate.unwrap_or(rate) {
            notices.push(format!(
                "Sample rate converted from {} to {rate} Hz",
                p.sample_rate.unwrap()
            ));
        }
        if delta != 0 {
            notices.push(format!("Decoded timing differs by {delta} frames after codec padding handling; inspect clip boundaries"));
        }
        Ok(ImageResult {
            properties: after,
            diagnostics: Diagnostics {
                duration_delta: Some(delta as f64 / rate as f64),
                notices,
                ..Default::default()
            },
        })
    }
    fn probe_json(&self, path: &Path, cancel: &AtomicBool) -> Result<serde_json::Value> {
        let mut args = strings(&[
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ]);
        args.push(path.to_string_lossy().into());
        serde_json::from_slice(&process::run(
            &self.tools.ffprobe,
            &args,
            None,
            cancel,
            Some(Duration::from_secs(30)),
        )?)
        .context("Invalid media probe")
    }
    fn pcm_path(&self, path: &Path, rate: u32, cancel: &AtomicBool) -> Result<PathBuf> {
        let props = probe_audio(&self.tools, path, cancel)?;
        let needed =
            (props.duration.unwrap_or(0.) * rate as f64 * props.channels.unwrap_or(1) as f64 * 4.)
                as u64;
        if fs2::available_space(&self.root)? < needed.saturating_add(16 * 1024 * 1024) {
            bail!("Not enough temporary disk space for decoded audio");
        }
        let temp = self
            .root
            .join("work")
            .join(format!("pcm-{}.f32", Uuid::new_v4()));
        let mut args = strings(&[
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]);
        args.push(path.to_string_lossy().into());
        args.extend(strings(&["-map", "0:a:0", "-vn", "-sn", "-dn", "-ar"]));
        args.push(rate.to_string());
        args.extend(strings(&[
            "-c:a",
            "pcm_f32le",
            "-f",
            "f32le",
            "-threads",
            "1",
            "-n",
        ]));
        args.push(temp.to_string_lossy().into());
        if let Err(e) = process::run(&self.tools.ffmpeg, &args, None, cancel, None) {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        Ok(temp)
    }
    fn decoded_frames(&self, path: &Path, rate: u32, cancel: &AtomicBool) -> Result<u64> {
        let props = probe_audio(&self.tools, path, cancel)?;
        let pcm = self.pcm_path(path, rate, cancel)?;
        let bytes = fs::metadata(&pcm)?.len();
        fs::remove_file(pcm)?;
        Ok(bytes / (4 * props.channels.unwrap_or(1) as u64))
    }
    fn pcm_hash(&self, path: &Path, rate: u32, cancel: &AtomicBool) -> Result<String> {
        let pcm = self.pcm_path(path, rate, cancel)?;
        let hash = hash_file(&pcm)?;
        fs::remove_file(pcm)?;
        Ok(hash)
    }
    pub fn cancel(&self, id: Option<&str>) {
        for (key, flag) in self.cancels.lock().unwrap().iter() {
            if id.is_none_or(|v| v == key) {
                flag.store(true, Ordering::Relaxed);
            }
        }
        for job in &mut self.data.lock().unwrap().jobs {
            if job.state == "queued" && id.is_none_or(|v| v == job.id) {
                job.state = "canceled".into();
                job.stage = "Canceled before encoding".into();
            }
        }
        self.changed();
    }
    pub fn export(&self, ids: &[String], destination: &Path, report: bool) -> Result<Vec<String>> {
        fs::create_dir_all(destination)?;
        let destination = destination.canonicalize()?;
        let snapshot = self.snapshot();
        let mut results = Vec::new();
        for media in &snapshot.media {
            for candidate in &media.candidates {
                if !ids.contains(&candidate.id) {
                    continue;
                }
                if hash_file(&media.path)? != media.sha256 {
                    bail!("{} changed; import again before export", media.name);
                }
                if hash_file(&candidate.path)? != candidate.sha256 {
                    bail!("Candidate integrity check failed");
                }
                let relative = Path::new(&media.relative_name);
                if relative
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                {
                    bail!("Unsafe export path");
                }
                let parent = safe_parent(&destination, relative.parent().unwrap_or(Path::new("")))?;
                if fs2::available_space(&parent)? < candidate.bytes {
                    bail!("Not enough export disk space");
                }
                let stem = relative.file_stem().unwrap_or_default().to_string_lossy();
                let extension = extension(&candidate.settings[0].format);
                let mut stage = tempfile::NamedTempFile::new_in(&parent)?;
                std::io::copy(&mut File::open(&candidate.path)?, &mut stage)?;
                stage.as_file().sync_all()?;
                if hash_file(stage.path())? != candidate.sha256 {
                    bail!("Candidate changed during export; staged output discarded");
                }
                if hash_file(&media.path)? != media.sha256 {
                    bail!("Source changed during export; staged output discarded");
                }
                let mut index = 1;
                let exported = loop {
                    let name = if index == 1 {
                        format!("{stem}-compressed.{extension}")
                    } else {
                        format!("{stem}-compressed-{index}.{extension}")
                    };
                    let target = parent.join(name);
                    match stage.persist_noclobber(&target) {
                        Ok(_) => break target,
                        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                            stage = e.file;
                            index += 1;
                        }
                        Err(e) => return Err(e.error.into()),
                    }
                };
                let relative_output = exported
                    .strip_prefix(&destination)?
                    .to_string_lossy()
                    .into_owned();
                results.push(relative_output.clone());
                if let Some(c) = self
                    .data
                    .lock()
                    .unwrap()
                    .media
                    .iter_mut()
                    .flat_map(|m| m.candidates.iter_mut())
                    .find(|c| c.id == candidate.id)
                {
                    c.exported.push(relative_output);
                }
                self.changed();
            }
        }
        if report {
            let snapshot = self.snapshot();
            let mut value = serde_json::to_value(&snapshot)?;
            for m in value["media"].as_array_mut().unwrap() {
                m.as_object_mut().unwrap().remove("preview");
                for c in m["candidates"].as_array_mut().unwrap() {
                    c.as_object_mut().unwrap().remove("preview");
                }
            }
            let paths = snapshot
                .media
                .iter()
                .map(|m| {
                    (
                        m.path.to_string_lossy().into_owned(),
                        m.relative_name.clone(),
                    )
                })
                .chain(std::iter::once((
                    self.root.to_string_lossy().into_owned(),
                    "[workspace]".into(),
                )))
                .collect::<Vec<_>>();
            redact_paths(&mut value, &paths);
            let text = serde_json::to_string_pretty(
                &serde_json::json!({"schemaVersion":1,"applicationVersion":env!("CARGO_PKG_VERSION"),"results":value,"exported":results}),
            )?;
            let mut stage = tempfile::NamedTempFile::new_in(&destination)?;
            stage.write_all(text.as_bytes())?;
            stage.as_file().sync_all()?;
            let target = destination.join(format!("compression-report-{}.json", Uuid::new_v4()));
            stage.persist_noclobber(target)?;
        }
        self.changed();
        Ok(results)
    }
    pub fn preview(&self, id: &str) -> Result<String> {
        let data = self.snapshot();
        let (path, props, hash) = if let Some(m) = data.media.iter().find(|m| m.id == id) {
            (
                m.path.clone(),
                m.properties.clone().context("Unsupported image")?,
                m.sha256.clone(),
            )
        } else {
            let c = data
                .media
                .iter()
                .flat_map(|m| &m.candidates)
                .find(|c| c.id == id)
                .context("Candidate not found")?;
            (c.path.clone(), c.properties.clone(), c.sha256.clone())
        };
        if props.kind != "image" {
            bail!("Choose an image");
        }
        if hash_file(&path)? != hash {
            bail!("Image changed; import again");
        }
        let output = self.root.join("work").join(format!("preview-{hash}.png"));
        if !output.exists() {
            self.image_request("probe", &path, &output, None, &AtomicBool::new(false))?;
        }
        Ok(output.to_string_lossy().into())
    }
    pub fn playback_pcm(&self, id: &str) -> Result<(PathBuf, u32, u16)> {
        let data = self.snapshot();
        let (path, p) = if let Some(m) = data.media.iter().find(|m| m.id == id) {
            (
                m.path.clone(),
                m.properties.clone().context("Unsupported audio")?,
            )
        } else {
            let c = data
                .media
                .iter()
                .flat_map(|m| &m.candidates)
                .find(|c| c.id == id)
                .context("Candidate not found")?;
            (c.path.clone(), c.properties.clone())
        };
        if p.kind != "audio" {
            bail!("Choose audio to play");
        }
        let rate = p.sample_rate.unwrap_or(48000);
        let channels = p.channels.unwrap_or(1);
        let hash = hash_file(&path)?;
        let dest = self
            .root
            .join("work")
            .join(format!("playback-{hash}-{rate}.f32"));
        if !dest.exists() {
            if fs2::available_space(&self.root)?
                < (p.duration.unwrap_or(0.) * rate as f64 * channels as f64 * 4.) as u64
                    + 16 * 1024 * 1024
            {
                bail!("Not enough playback cache disk space");
            }
            let pcm = self.pcm_path(&path, rate, &AtomicBool::new(false))?;
            fs::rename(pcm, &dest)?;
        }
        Ok((dest, rate, channels))
    }
    pub fn clear(&self) -> Result<()> {
        if self.busy.load(Ordering::SeqCst) {
            bail!("Cancel or finish processing before clearing");
        }
        for name in ["work", "cache"] {
            let dir = self.root.join(name);
            fs::remove_dir_all(&dir)?;
            fs::create_dir_all(dir)?;
        }
        let tools = self.data.lock().unwrap().tools.clone();
        *self.data.lock().unwrap() = Snapshot {
            tools,
            ..Default::default()
        };
        self.changed();
        Ok(())
    }
}
fn redact_paths(value: &mut serde_json::Value, paths: &[(String, String)]) {
    match value {
        serde_json::Value::String(text) => {
            for (source, label) in paths {
                *text = text
                    .replace(source, label)
                    .replace(&source.replace('\\', "/"), label);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_paths(item, paths);
            }
        }
        serde_json::Value::Object(items) => {
            for item in items.values_mut() {
                redact_paths(item, paths);
            }
        }
        _ => {}
    }
}
fn safe_parent(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut parent = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            bail!("Unsafe export path");
        };
        parent.push(name);
        match fs::symlink_metadata(&parent) {
            Ok(metadata) => {
                let redirect = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let redirect = {
                    use std::os::windows::fs::MetadataExt;
                    redirect || metadata.file_attributes() & 0x400 != 0
                };
                if redirect || !metadata.is_dir() {
                    bail!("Export subfolder is a link or is not a directory");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&parent)?;
            }
            Err(error) => return Err(error.into()),
        }
        parent = parent.canonicalize()?;
        if !parent.starts_with(root) {
            bail!("Export folder resolves outside the destination");
        }
    }
    Ok(parent)
}

fn files_too_many(n: usize) -> bool {
    n >= 10000
}
fn collect_files(
    root: &Path,
    dir: &Path,
    prefix: &str,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            collect_files(root, &path, prefix, files)?;
        } else {
            files.push((
                path.clone(),
                format!("{prefix}/{}", path.strip_prefix(root)?.to_string_lossy()),
            ));
        }
        if files.len() > 10000 {
            bail!("Folder contains more than 10,000 files; select smaller batches");
        }
    }
    Ok(())
}
pub fn probe_audio(tools: &Tools, path: &Path, cancel: &AtomicBool) -> Result<Properties> {
    let mut args = strings(&[
        "-v",
        "error",
        "-protocol_whitelist",
        "file,pipe",
        "-show_format",
        "-show_streams",
        "-of",
        "json",
    ]);
    args.push(path.to_string_lossy().into());
    let data: serde_json::Value = serde_json::from_slice(&process::run(
        &tools.ffprobe,
        &args,
        None,
        cancel,
        Some(Duration::from_secs(30)),
    )?)?;
    let streams = data["streams"]
        .as_array()
        .context("Unsupported or corrupted media")?;
    if streams.len() == 1
        && ["png", "mjpeg", "webp"].contains(&streams[0]["codec_name"].as_str().unwrap_or(""))
    {
        return Ok(Properties {
            kind: "image".into(),
            ..Default::default()
        });
    }
    if streams
        .iter()
        .any(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
    {
        bail!("Video and animated media are outside this release");
    }
    let stream = streams
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .context("No supported audio stream")?;
    let codec = stream["codec_name"].as_str().unwrap_or("");
    if !["aac", "mp3", "flac", "opus", "vorbis"].contains(&codec) && !codec.starts_with("pcm_") {
        bail!("Audio codec {codec} is not supported");
    }
    if streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .count()
        != 1
    {
        bail!("Multiple audio tracks are outside this release");
    }
    let channels = stream["channels"]
        .as_u64()
        .context("Unknown audio channel layout")?;
    if !(1..=2).contains(&channels) {
        bail!("Only mono and stereo audio are supported");
    }
    let rate = stream["sample_rate"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .context("Unknown sample rate")?;
    let duration = stream["duration"]
        .as_str()
        .or(data["format"]["duration"].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .context("Unknown audio duration")?;
    if !duration.is_finite() || duration <= 0. {
        bail!("Audio duration is invalid");
    }
    let bits = stream["bits_per_raw_sample"]
        .as_str()
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|v| *v > 0)
        .or_else(|| {
            stream["bits_per_sample"]
                .as_u64()
                .filter(|v| *v > 0)
                .map(|v| v as u8)
        });
    Ok(Properties {
        kind: "audio".into(),
        format: if codec.starts_with("pcm_") {
            "wav"
        } else {
            codec
        }
        .into(),
        sample_rate: Some(rate),
        channels: Some(channels as u16),
        duration: Some(duration),
        bit_depth: bits,
        sample_format: stream["sample_fmt"].as_str().map(str::to_string),
        ..Default::default()
    })
}

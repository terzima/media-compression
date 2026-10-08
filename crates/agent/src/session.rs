use anyhow::{bail, Context, Result};
use media_engine::{Engine, Settings, Snapshot, Tools};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};

pub const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct Scope {
    pub roots: Vec<PathBuf>,
}
impl Scope {
    pub fn new(roots: Vec<PathBuf>) -> Result<Self> {
        if roots.is_empty() {
            bail!("Grant at least one folder with --root <folder>");
        }
        let roots = roots
            .into_iter()
            .map(|p| {
                let p = p.canonicalize().context("Granted folder does not exist")?;
                if !p.is_dir() {
                    bail!("Granted roots must be folders");
                }
                Ok(p)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { roots })
    }
    // Resolve existing ancestors, including links/junctions, before creating output directories.
    pub fn path(&self, path: &Path) -> Result<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let mut existing = absolute.clone();
        let mut tail = Vec::new();
        while !existing.exists() {
            let name = existing.file_name().context("Invalid path")?.to_os_string();
            if fs::symlink_metadata(&existing).is_ok() {
                bail!("Broken links are not allowed");
            }
            tail.push(name);
            if !existing.pop() {
                bail!("No existing parent");
            }
        }
        let mut resolved = existing.canonicalize()?;
        if !self.roots.iter().any(|r| resolved.starts_with(r)) {
            bail!("Path is outside the granted folders");
        }
        for part in tail.into_iter().rev() {
            if !Path::new(&part)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            {
                bail!("Unsafe path component");
            }
            resolved.push(part);
        }
        Ok(resolved)
    }
}

pub fn read_json(path: &Path) -> Result<Value> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_JSON_BYTES {
        bail!("JSON input exceeds 64 MiB");
    }
    let mut bytes = Vec::new();
    file.take(MAX_JSON_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        bail!("JSON input exceeds 64 MiB");
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Choose a manifest filename")?;
    fs::create_dir_all(parent)?;
    let mut stage = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut stage, value)?;
    stage.write_all(b"\n")?;
    stage.as_file().sync_all()?;
    stage.persist_noclobber(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn bundled_tools() -> Result<Tools> {
    let exe = std::env::current_exe()?;
    let bin = exe.parent().context("Executable folder missing")?;
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let codecs = std::env::var_os("MEDIA_CODEC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                bin.join("../Resources/resources/codecs")
            } else {
                bin.join("resources/codecs")
            }
        });
    let worker = std::env::var_os("MEDIA_WORKER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| bin.join(format!("media-worker{ext}")));
    Ok(Tools {
        worker,
        ffmpeg: codecs.join(format!("ffmpeg{ext}")),
        ffprobe: codecs.join(format!("ffprobe{ext}")),
        pngquant: codecs.join(format!("pngquant{ext}")),
        cwebp: codecs.join(format!("cwebp{ext}")),
        cjpeg: codecs.join(format!("cjpeg{ext}")),
    })
}

pub struct Session {
    pub engine: Arc<Engine>,
    pub scope: Scope,
    inputs: std::sync::Mutex<Vec<PathBuf>>,
    pub(crate) workspace: PathBuf,
    operation: Mutex<()>,
    pub(crate) folder_active: AtomicBool,
    pub(crate) folder_canceled: AtomicBool,
}
impl Session {
    pub fn new(workspace: PathBuf, scope: Scope, tools: Tools) -> Result<Self> {
        let marker = workspace.join("agent-workspace-v1.json");
        if marker.exists() {
            let value = read_json(&marker)?;
            if fs::symlink_metadata(&marker)?.file_type().is_symlink()
                || value["schemaVersion"] != 1
                || value["owner"] != "media-compression-agent"
            {
                bail!("Invalid agent workspace ownership record");
            }
        }
        // Engine startup cleans its work/ directory. Never do that to an arbitrary pre-existing folder.
        if workspace.exists()
            && !workspace.join("agent-workspace-v1.json").is_file()
            && fs::read_dir(&workspace)?.next().is_some()
        {
            bail!("Agent workspace must be empty or owned by Media Compression");
        }
        fs::create_dir_all(&workspace)?;
        let engine = Engine::new(workspace.clone(), tools, || {})?;
        if !workspace.join("agent-workspace-v1.json").exists() {
            write_json_new(
                &workspace.join("agent-workspace-v1.json"),
                &json!({"schemaVersion":1,"owner":"media-compression-agent"}),
            )?;
        }
        Ok(Self {
            engine,
            scope,
            inputs: std::sync::Mutex::new(Vec::new()),
            workspace,
            operation: Mutex::new(()),
            folder_active: AtomicBool::new(false),
            folder_canceled: AtomicBool::new(false),
        })
    }
    pub fn import(&self, paths: Vec<PathBuf>) -> Result<()> {
        let _operation = self.operation()?;
        if paths.is_empty() {
            bail!("Provide at least one input path");
        }
        let paths = paths
            .iter()
            .map(|p| self.scope.path(p))
            .collect::<Result<Vec<_>>>()?;
        self.engine
            .import_scoped(paths.clone(), &self.scope.roots)?;
        let mut inputs = self.inputs.lock().unwrap();
        for p in paths {
            if !inputs.contains(&p) {
                inputs.push(p);
            }
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Value {
        snapshot_json(self.engine.snapshot())
    }
    pub fn active(&self) -> bool {
        self.folder_active.load(Ordering::SeqCst)
            || self
                .engine
                .snapshot()
                .jobs
                .iter()
                .any(|j| ["queued", "processing"].contains(&j.state.as_str()))
    }
    pub async fn wait(&self) {
        while self.active() {
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    }
    pub fn save(&self, path: &Path) -> Result<Value> {
        let _operation = self.operation()?;
        if self.active() {
            bail!("Wait for processing or cancellation to finish before saving the study");
        }
        let snapshot = self.engine.snapshot();
        let sources = snapshot
            .media
            .iter()
            .map(|m| SavedSource {
                id: m.id.clone(),
                path: m.path.clone(),
                relative_name: m.relative_name.clone(),
                sha256: m.sha256.clone(),
                candidates: m
                    .candidates
                    .iter()
                    .map(|c| SavedCandidate {
                        id: c.id.clone(),
                        sha256: c.sha256.clone(),
                        settings: c.settings.clone(),
                    })
                    .collect(),
            })
            .collect();
        let manifest = Manifest {
            schema_version: 1,
            application_version: env!("CARGO_PKG_VERSION").into(),
            inputs: self.inputs.lock().unwrap().clone(),
            sources,
            results: snapshot_json(snapshot),
        };
        let path = self.scope.path(path)?;
        write_json_new(&path, &manifest)?;
        Ok(json!({"schemaVersion":1,"manifest":path}))
    }
    pub fn export(&self, ids: Vec<String>, destination: &Path, report: bool) -> Result<Value> {
        let _operation = self.operation()?;
        if ids.is_empty() {
            bail!("Select at least one candidate ID");
        }
        let snapshot = self.engine.snapshot();
        for id in &ids {
            let media = snapshot
                .media
                .iter()
                .find(|m| m.candidates.iter().any(|c| &c.id == id))
                .context("Unknown candidate ID")?;
            self.scope.path(&media.path)?;
        }
        let destination = self.scope.path(destination)?;
        let prior_exports: HashMap<_, _> = snapshot
            .media
            .iter()
            .flat_map(|m| &m.candidates)
            .map(|c| (c.id.clone(), c.exported.len()))
            .collect();
        let result = self.engine.export(&ids, &destination, report);
        let after = self.engine.snapshot();
        let exports: Vec<_> = after.media.iter().flat_map(|m| &m.candidates).filter(|c| ids.contains(&c.id)).flat_map(|c| {
            c.exported.iter().skip(prior_exports.get(&c.id).copied().unwrap_or(0)).map(|relative| json!({"candidateId":c.id,"path":destination.join(relative),"bytes":c.bytes,"sha256":c.sha256}))
        }).collect();
        let mut value = snapshot_json(after);
        value["destination"] = json!(destination);
        value["exports"] = json!(exports);
        value["exportError"] = result
            .err()
            .map(|e| json!(format!("{e:#}")))
            .unwrap_or(Value::Null);
        Ok(value)
    }
    pub async fn restore_export(
        &self,
        manifest_path: &Path,
        selected: Vec<String>,
        destination: &Path,
        report: bool,
    ) -> Result<Value> {
        let manifest: Manifest =
            serde_json::from_value(read_json(&self.scope.path(manifest_path)?)?)?;
        if manifest.schema_version != 1 {
            bail!("Unsupported study manifest version");
        }
        if selected.is_empty() {
            bail!("Select candidate IDs from the saved study");
        }
        let mut wanted = Vec::new();
        for id in &selected {
            let (source, candidate) = manifest
                .sources
                .iter()
                .find_map(|s| s.candidates.iter().find(|c| &c.id == id).map(|c| (s, c)))
                .context("Candidate ID missing from study manifest")?;
            self.scope.path(&source.path)?;
            wanted.push((source.clone(), candidate.clone()));
        }
        let mut paths = Vec::new();
        for (source, _) in &wanted {
            if !paths.contains(&source.path) {
                paths.push(source.path.clone());
            }
        }
        self.import(paths)?;
        let snapshot = self.engine.snapshot();
        let mut errors = Vec::new();
        let mut jobs: std::collections::BTreeMap<String, Vec<Settings>> = Default::default();
        for (s, c) in &wanted {
            let source = snapshot
                .media
                .iter()
                .find(|m| m.path == s.path && m.error.is_none() && m.sha256 == s.sha256);
            if let Some(m) = source {
                if !s.relative_name.is_empty() {
                    self.engine.restore_import_name(&m.id, &s.relative_name)?;
                }
                let settings = jobs.entry(m.id.clone()).or_default();
                for setting in &c.settings {
                    if !settings.contains(setting) {
                        settings.push(setting.clone());
                    }
                }
            } else {
                errors.push(format!("{}: source changed, missing or unsupported", s.id));
            }
        }
        if !jobs.is_empty() {
            self.start(jobs.into_iter().collect())?;
            self.wait().await;
        }
        let snapshot = self.engine.snapshot();
        let mut ids = Vec::new();
        for (s, c) in wanted {
            if let Some(found) = snapshot
                .media
                .iter()
                .filter(|m| m.path == s.path && m.sha256 == s.sha256)
                .flat_map(|m| &m.candidates)
                .find(|actual| {
                    actual.sha256 == c.sha256
                        && actual.settings.iter().any(|a| c.settings.contains(a))
                })
            {
                if !ids.contains(&found.id) {
                    ids.push(found.id.clone());
                }
            } else if !errors.iter().any(|e| e.starts_with(&s.id)) {
                errors.push(format!(
                    "{}: candidate cannot be verified with current codecs/settings",
                    c.id
                ));
            }
        }
        let mut result = if ids.is_empty() {
            self.snapshot()
        } else {
            self.export(ids, destination, report)?
        };
        result["restoreErrors"] = json!(errors);
        Ok(result)
    }
    pub(crate) fn operation(&self) -> Result<MutexGuard<'_, ()>> {
        self.operation.try_lock().map_err(|_| {
            anyhow::anyhow!(
                "Another import, export or folder operation is active; wait before changing this session"
            )
        })
    }
    pub fn start(&self, items: Vec<(String, Vec<Settings>)>) -> Result<Vec<String>> {
        let _operation = self.operation()?;
        self.engine.start(items)
    }
    pub fn cancel(&self, id: Option<&str>) {
        if id.is_none() {
            self.folder_canceled.store(true, Ordering::SeqCst);
        }
        self.engine.cancel(id);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel(None);
    }
}

pub fn snapshot_json(snapshot: Snapshot) -> Value {
    let mut value = serde_json::to_value(&snapshot).expect("Serializable snapshot");
    value["schemaVersion"] = json!(1);
    for (row, m) in value["media"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&snapshot.media)
    {
        for candidate in row["candidates"].as_array_mut().unwrap() {
            let bytes = candidate["bytes"].as_u64().unwrap_or(0);
            candidate["bytesSaved"] = json!(m.bytes as i128 - bytes as i128);
            candidate["savingsPercent"] = if m.bytes == 0 {
                Value::Null
            } else {
                json!((1. - bytes as f64 / m.bytes as f64) * 100.)
            };
        }
    }
    value
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedCandidate {
    pub id: String,
    pub sha256: String,
    pub settings: Vec<Settings>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedSource {
    pub id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub relative_name: String,
    pub sha256: String,
    pub candidates: Vec<SavedCandidate>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub application_version: String,
    pub inputs: Vec<PathBuf>,
    pub sources: Vec<SavedSource>,
    pub results: Value,
}

pub fn has_errors(value: &Value) -> bool {
    value["errors"].as_array().is_some_and(|a| !a.is_empty())
        || value["media"].as_array().is_some_and(|a| {
            a.iter().any(|m| {
                !m["error"].is_null()
                    || m["candidates"].as_array().is_some_and(|c| {
                        c.iter()
                            .any(|v| v["exportErrors"].as_array().is_some_and(|e| !e.is_empty()))
                    })
            })
        })
        || value["jobs"].as_array().is_some_and(|a| {
            a.iter().any(|j| {
                j["errors"].as_array().is_some_and(|e| !e.is_empty()) || j["state"] == "canceled"
            })
        })
        || !value["exportError"].is_null()
        || value["restoreErrors"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
}

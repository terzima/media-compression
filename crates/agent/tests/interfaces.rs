use media_engine::hash_file;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

fn program() -> PathBuf {
    std::env::var_os("MEDIA_AGENT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_media-compression-agent")))
}
fn command(root: &Path, workspace: &Path, mode: &str) -> Command {
    let mut command = Command::new(program());
    command
        .args([mode, "--root"])
        .arg(root)
        .arg("--workspace")
        .arg(workspace);
    if std::env::var_os("MEDIA_AGENT_PATH").is_some() {
        command
            .env_remove("MEDIA_CODEC_DIR")
            .env_remove("MEDIA_WORKER_PATH");
    } else {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let target = std::env::var("MEDIA_BUILD_TARGET").unwrap_or_else(|_| {
            if cfg!(windows) {
                "x86_64-pc-windows-msvc"
            } else if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
            .into()
        });
        let ext = if cfg!(windows) { ".exe" } else { "" };
        command
            .env("MEDIA_CODEC_DIR", repo.join("src-tauri/resources/codecs"))
            .env(
                "MEDIA_WORKER_PATH",
                repo.join(format!("src-tauri/binaries/media-worker-{target}{ext}")),
            );
    }
    command
}
fn fixture(root: &Path) -> PathBuf {
    let file = root.join("original 日本 🐈.png");
    image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([
            (x * 4) as u8,
            (y * 4) as u8,
            ((x + y) * 2) as u8,
            if x < 10 { 0 } else { 255 },
        ])
    })
    .save(&file)
    .unwrap();
    file
}
fn run(
    root: &Path,
    workspace: &Path,
    mode: &str,
    request: Value,
    manifest: Option<&Path>,
) -> (i32, Value) {
    let input = root.join(format!("request-{mode}.json"));
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let mut command = command(root, workspace, mode);
    command.arg("--input").arg(input);
    if let Some(m) = manifest {
        command.arg("--manifest").arg(m);
    }
    let output = command.output().unwrap();
    let bytes = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let result = serde_json::from_slice(bytes).unwrap_or_else(|e| {
        panic!(
            "{e}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), result)
}
struct Client {
    child: Child,
    input: Option<ChildStdin>,
    responses: mpsc::Receiver<Value>,
    id: u64,
}
impl Client {
    fn new(root: &Path, workspace: &Path) -> Self {
        let mut child = command(root, workspace, "mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = child.stdout.take().unwrap();
        let (tx, responses) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    if tx.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let mut client = Self {
            child,
            input,
            responses,
            id: 0,
        };
        let result=client.request("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"media-compression-test","version":"1"}}));
        assert_eq!(result["result"]["serverInfo"]["name"], "media-compression");
        client.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        client
    }
    fn send(&mut self, value: Value) {
        let stdin = self.input.as_mut().unwrap();
        writeln!(stdin, "{value}").unwrap();
        stdin.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let value = self
                .responses
                .recv_timeout(Duration::from_secs(30))
                .expect("MCP response timeout/disconnect");
            if value["id"] == id {
                return value;
            }
        }
    }
    fn call(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":args}))["result"].clone()
    }
    fn close(&mut self) {
        self.input.take();
        let start = Instant::now();
        while self.child.try_wait().unwrap().is_none() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Server did not exit on stdio EOF"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn scoped_paths_and_workspace_ownership_are_checked() {
    use media_agent::session::Scope;
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let scope = Scope::new(vec![dir.path().into()]).unwrap();
    assert!(scope.path(&dir.path().join("future/output")).is_ok());
    assert!(scope.path(&other.path().join("future")).is_err());
    assert!(scope
        .path(&dir.path().join("../escape-new/output"))
        .is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(other.path(), dir.path().join("redirect")).unwrap();
        assert!(scope.path(&dir.path().join("redirect/output")).is_err());
    }
    use media_agent::session::Session;
    let workspace = dir.path().join("unowned");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("precious.txt"), "keep").unwrap();
    let missing = media_engine::Tools {
        worker: PathBuf::new(),
        ffmpeg: PathBuf::new(),
        ffprobe: PathBuf::new(),
        pngquant: PathBuf::new(),
        cwebp: PathBuf::new(),
        cjpeg: PathBuf::new(),
    };
    assert!(Session::new(workspace.clone(), scope.clone(), missing.clone()).is_err());
    std::fs::write(workspace.join("agent-workspace-v1.json"), "{}").unwrap();
    assert!(Session::new(workspace.clone(), scope, missing).is_err());
    assert_eq!(
        std::fs::read_to_string(workspace.join("precious.txt")).unwrap(),
        "keep"
    );
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn cli_study_export_and_stale_source_rejection() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let source = fixture(root);
    let original_hash = hash_file(&source).unwrap();
    let workspace = root.join("agent cache 🐈");
    let manifest = root.join("study.json");
    let (code, result) = run(
        root,
        &workspace,
        "study",
        json!({"paths":[source],"settings":[{"format":"webp","lossless":false,"quality":0},{"format":"webp","lossless":false,"quality":80},{"format":"webp","lossless":false,"quality":80}]}),
        Some(&manifest),
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(
        result["media"][0]["candidates"].as_array().unwrap().len(),
        2
    );
    let candidate = result["media"][0]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["settings"][0]["quality"].as_f64() == Some(0.))
        .unwrap();
    let id = candidate["id"].as_str().unwrap();
    let hash = candidate["sha256"].as_str().unwrap();
    let destination = root.join("exports");
    let request =
        json!({"study":manifest,"candidateIds":[id],"destination":destination,"report":true});
    let (code, export) = run(root, &workspace, "export", request.clone(), None);
    assert_eq!(code, 0, "{export}");
    let output = export["media"][0]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["sha256"] == hash)
        .unwrap()["exported"][0]
        .as_str()
        .unwrap();
    assert_eq!(hash_file(&destination.join(output)).unwrap(), hash);
    assert_eq!(hash_file(&source).unwrap(), original_hash);
    let (code, again) = run(root, &workspace, "export", request.clone(), None);
    assert_eq!(code, 0, "{again}");
    assert_eq!(
        std::fs::read_dir(&destination)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|e| e == "webp"))
            .count(),
        2
    );
    std::fs::write(&source, b"changed original").unwrap();
    let (code, stale) = run(root, &workspace, "export", request, None);
    assert_eq!(code, 2, "{stale}");
    assert!(!stale["restoreErrors"].as_array().unwrap().is_empty());
    let (code, invalid) = run(
        root,
        &workspace,
        "study",
        json!({"paths":[source],"settings":[{"format":"webp","lossless":true,"effrt":6}]}),
        Some(&root.join("invalid.json")),
    );
    assert_eq!(code, 1, "{invalid}");
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn cli_saved_folder_export_reopens_only_selected_sources() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("photos");
    let nested = folder.join("日本 🐈");
    std::fs::create_dir_all(&nested).unwrap();
    let selected = fixture(&nested);
    let unselected = fixture(&folder);
    let original_hash = hash_file(&selected).unwrap();
    let workspace = root.join("cache");
    let manifest = root.join("folder-study.json");
    let (code, study) = run(
        root,
        &workspace,
        "study",
        json!({"paths":[folder],"settings":[{"format":"webp","lossless":true}]}),
        Some(&manifest),
    );
    assert_eq!(code, 0, "{study}");
    let media = study["media"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["relativeName"].as_str().unwrap().contains("日本 🐈"))
        .unwrap();
    let candidate = &media["candidates"][0];
    let id = candidate["id"].as_str().unwrap();
    let hash = candidate["sha256"].as_str().unwrap();
    // A later selected export must not re-probe unrelated files from the old folder import.
    std::fs::write(&unselected, b"now corrupt").unwrap();
    let destination = root.join("exports");
    let (code, export) = run(
        root,
        &workspace,
        "export",
        json!({"study":manifest,"candidateIds":[id],"destination":destination}),
        None,
    );
    assert_eq!(code, 0, "{export}");
    assert_eq!(export["media"].as_array().unwrap().len(), 1);
    let exported = export["media"][0]["candidates"][0]["exported"][0]
        .as_str()
        .unwrap();
    assert!(
        Path::new(exported).starts_with(Path::new("photos").join("日本 🐈")),
        "{exported}"
    );
    assert_eq!(hash_file(&destination.join(exported)).unwrap(), hash);
    assert_eq!(hash_file(&selected).unwrap(), original_hash);
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn mcp_discovers_tools_studies_previews_exports_and_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let source = fixture(root);
    let hash = hash_file(&source).unwrap();
    let wav = root.join("audio.wav");
    let mut writer = hound::WavWriter::create(
        &wav,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..4800 {
        writer
            .write_sample(((i as f32 / 8.).sin() * 10000.) as i16)
            .unwrap();
    }
    writer.finalize().unwrap();
    let mut client = Client::new(root, &root.join("mcp-cache"));
    let tools = client.request("tools/list", json!({}));
    assert!(tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "plan_study"));
    let denied = client.call(
        "import_media",
        json!({"paths":[std::env::temp_dir().parent().unwrap()]}),
    );
    assert_eq!(denied["isError"], true);
    let imported = client.call("import_media", json!({"paths":[source,wav]}));
    assert_eq!(imported["isError"], false, "{imported}");
    let media = imported["structuredContent"]["media"].as_array().unwrap();
    let id = media
        .iter()
        .find(|m| m["properties"]["kind"] == "image")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let audio_id = media
        .iter()
        .find(|m| m["properties"]["kind"] == "audio")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let items = json!({"items":[{"mediaId":id,"settings":[{"format":"webp","lossless":true}]},{"mediaId":audio_id,"settings":[{"format":"flac","lossless":true,"effort":5}]}]});
    let plan = client.call("plan_study", items.clone());
    assert_eq!(plan["structuredContent"]["eligibleEncodes"], 2);
    assert_eq!(
        client.call("job_status", json!({}))["structuredContent"]["jobs"],
        json!([])
    );
    let started = client.call("start_study", items);
    assert_eq!(started["isError"], false, "{started}");
    let start = Instant::now();
    loop {
        let status = client.call("job_status", json!({}));
        if status["structuredContent"]["active"] == false {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(60));
    }
    let result = client.call("get_candidates", json!({"mediaId":id}));
    let candidate = &result["structuredContent"]["media"]["candidates"][0];
    assert_eq!(candidate["diagnostics"]["pixelIdentical"], true);
    let candidate_id = candidate["id"].as_str().unwrap().to_owned();
    let preview = client.call("preview_image", json!({"id":candidate_id}));
    assert!(preview["content"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["type"] == "image"));
    let audio = client.call("get_candidates", json!({"mediaId":audio_id}));
    assert!(!audio["structuredContent"]["media"]["candidates"]
        .as_array()
        .unwrap()
        .is_empty());
    let exported = client.call(
        "export_candidates",
        json!({"candidateIds":[candidate_id],"destination":root.join("output"),"report":true}),
    );
    assert_eq!(exported["isError"], false, "{exported}");
    assert_eq!(
        client.call("save_study", json!({"path":root.join("mcp-study.json")}))["isError"],
        false
    );
    let many: Vec<_> = (0..512)
        .map(|i| json!({"format":"webp","lossless":false,"quality":i as f64/512.*100.}))
        .collect();
    let started = client.call(
        "start_study",
        json!({"items":[{"mediaId":id,"settings":many}]}),
    );
    assert_eq!(started["isError"], false, "{started}");
    let start = Instant::now();
    client.call("cancel_study", json!({}));
    loop {
        let status = client.call("job_status", json!({}));
        if status["structuredContent"]["active"] == false {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(40));
    }
    assert_eq!(hash_file(&source).unwrap(), hash);
    let settings: Vec<_> = (0..512)
        .map(|i| json!({"format":"webp","lossless":false,"quality":i as f64/512.*100.}))
        .collect();
    assert_eq!(
        client.call(
            "start_study",
            json!({"items":[{"mediaId":id,"settings":settings}]})
        )["isError"],
        false
    );
    client.close();
}

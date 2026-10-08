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
    pending: std::collections::HashMap<u64, Value>,
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
            pending: Default::default(),
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
        self.response(id)
    }
    fn response(&mut self, id: u64) -> Value {
        if let Some(value) = self.pending.remove(&id) {
            return value;
        }
        loop {
            let value = self
                .responses
                .recv_timeout(Duration::from_secs(30))
                .expect("MCP response timeout/disconnect");
            if value["id"] == id {
                return value;
            }
            if let Some(other) = value["id"].as_u64() {
                self.pending.insert(other, value);
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
fn setup_discovery_is_json_and_does_not_create_a_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let workspace = root.join("unused-cache");
    let mut results = Vec::new();
    for mode in ["guide", "capabilities", "tools", "config", "recipes"] {
        let output = command(&root, &workspace, mode).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        results.push(serde_json::from_slice::<Value>(&output.stdout).unwrap());
        assert!(
            !workspace.exists(),
            "Discovery must not start an engine/cache"
        );
    }
    assert!(results[0]["instructions"].as_str().unwrap().len() > 100);
    assert_eq!(results[1]["maxWorkers"], 2);
    assert_eq!(results[2]["tools"].as_array().unwrap().len(), 13);
    let server = &results[3]["mcpServers"]["media-compression"];
    assert!(Path::new(server["command"].as_str().unwrap()).is_file());
    assert_eq!(server["args"], json!(["mcp", "--root", root]));
    assert_eq!(results[4], media_agent::recipes::catalog(None).unwrap());
    let filtered = command(&root, &workspace, "recipes")
        .arg("audio-exact")
        .output()
        .unwrap();
    assert!(filtered.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&filtered.stdout).unwrap(),
        media_agent::recipes::catalog(Some("audio-exact")).unwrap()
    );
    let unknown = command(&root, &workspace, "recipes")
        .arg("video")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(1));
    assert!(serde_json::from_slice::<Value>(&unknown.stderr).unwrap()["error"].is_object());
    assert!(!workspace.exists());
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
    assert_eq!(export["exports"].as_array().unwrap().len(), 1);
    assert_eq!(export["exports"][0]["sha256"], hash);
    let absolute_output = Path::new(export["exports"][0]["path"].as_str().unwrap());
    assert!(absolute_output.is_absolute());
    assert_eq!(hash_file(absolute_output).unwrap(), hash);
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
    assert_eq!(again["exports"].as_array().unwrap().len(), 1);
    assert_ne!(again["exports"][0]["path"], export["exports"][0]["path"]);
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
        .find(|m| {
            Path::new(m["relativeName"].as_str().unwrap())
                .starts_with(Path::new("photos").join("日本 🐈"))
        })
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
    let audio_hash = hash_file(&wav).unwrap();
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
    let image_recipe = client.call("compression_recipes", json!({"recipeId":"image-exact"}));
    let audio_recipe = client.call("compression_recipes", json!({"recipeId":"audio-exact"}));
    assert_eq!(image_recipe["isError"], false);
    assert_eq!(audio_recipe["isError"], false);
    assert_eq!(
        client.call("compression_recipes", json!({"recipeId":"video"}))["isError"],
        true
    );
    assert_eq!(
        client.call("compression_recipes", json!({"unexpected":true}))["isError"],
        true
    );
    let image_setting =
        &image_recipe["structuredContent"]["recipes"][0]["variants"][1]["settings"][0];
    let audio_setting =
        &audio_recipe["structuredContent"]["recipes"][0]["variants"][0]["settings"][0];
    let items = json!({"items":[{"mediaId":id,"settings":[image_setting]},{"mediaId":audio_id,"settings":[audio_setting]}]});
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
    let audio_candidate = &audio["structuredContent"]["media"]["candidates"][0];
    assert_eq!(audio_candidate["diagnostics"]["samplesIdentical"], true);
    let exported = client.call(
        "export_candidates",
        json!({"candidateIds":[candidate_id,audio_candidate["id"]],"destination":root.join("output"),"report":true}),
    );
    assert_eq!(exported["isError"], false, "{exported}");
    let outputs = exported["structuredContent"]["exports"].as_array().unwrap();
    assert_eq!(outputs.len(), 2);
    for output in outputs {
        let path = Path::new(output["path"].as_str().unwrap());
        assert!(path.is_absolute() && path.is_file());
        assert_eq!(hash_file(path).unwrap(), output["sha256"]);
    }
    let again = client.call("export_candidates", json!({"candidateIds":[audio_candidate["id"]],"destination":root.join("second-output"),"report":false}));
    let outputs = again["structuredContent"]["exports"].as_array().unwrap();
    assert_eq!(outputs.len(), 1);
    let path = Path::new(outputs[0]["path"].as_str().unwrap());
    assert!(path.starts_with(root.join("second-output").canonicalize().unwrap()) && path.is_file());
    let held_audio = root.join("held-audio.wav");
    std::fs::rename(&wav, &held_audio).unwrap();
    let partial = client.call("export_candidates", json!({"candidateIds":[candidate_id,audio_candidate["id"]],"destination":root.join("partial-output"),"report":false}));
    std::fs::rename(&held_audio, &wav).unwrap();
    assert!(!partial["structuredContent"]["exportError"].is_null());
    let outputs = partial["structuredContent"]["exports"].as_array().unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0]["candidateId"], candidate_id);
    assert!(Path::new(outputs[0]["path"].as_str().unwrap()).is_file());
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
    assert_eq!(hash_file(&wav).unwrap(), audio_hash);
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

fn compress_folder(root: &Path, workspace: &Path, folder: &Path, mode: &str) -> (i32, Value) {
    let output = command(root, workspace, "compress-folder")
        .arg(folder)
        .args(["--mode", mode])
        .output()
        .unwrap();
    let bytes = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let value = serde_json::from_slice(bytes)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(bytes)));
    (output.status.code().unwrap(), value)
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn automatic_folder_mixed_outputs_cache_fallback_and_partial_errors() {
    let dir = tempfile::tempdir().unwrap();
    let canonical_root = dir.path().canonicalize().unwrap();
    let root = canonical_root.as_path();
    let folder = root.join("photos 日本 🐈");
    let nested = folder.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let source = fixture(&nested);
    // Include an already compact, tiny JPEG alongside high precision and audio.
    let jpeg = folder.join("tiny.jpg");
    image::RgbImage::new(1, 1).save(&jpeg).unwrap();
    let sixteen = folder.join("precision.png");
    image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(32, 32, |x, y| {
        image::Luma([(x * 37 + y * 67) as u16])
    })
    .save(&sixteen)
    .unwrap();
    let wav = folder.join("audio.wav");
    let mut w = hound::WavWriter::create(
        &wav,
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..13230 {
        w.write_sample(((i as f64 / 17.).sin() * 1_000_000.) as i32)
            .unwrap();
        w.write_sample(((i as f64 / 19.).cos() * 1_000_000.) as i32)
            .unwrap();
    }
    w.finalize().unwrap();
    let originals: Vec<_> = [&source, &jpeg, &sixteen, &wav]
        .iter()
        .map(|p| ((*p).clone(), hash_file(p).unwrap()))
        .collect();
    std::fs::write(folder.join("broken.png"), "corrupt").unwrap();
    std::fs::write(folder.join("notes.txt"), "ignore").unwrap();
    std::fs::create_dir(folder.join("Lossless")).unwrap();
    fixture(&folder.join("Lossless"));
    #[cfg(unix)]
    std::os::unix::fs::symlink(&nested, folder.join("link")).unwrap();
    let workspace = root.join("cache");
    let (code, result) = compress_folder(root, &workspace, &folder, "both");
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["sourceFiles"], 5);
    assert_eq!(result["plannedEncodes"], 7, "{result}");
    assert_eq!(result["outputs"].as_array().unwrap().len(), 8);
    assert!(!result["errors"].as_array().unwrap().is_empty());
    for row in result["outputs"].as_array().unwrap() {
        let output = Path::new(row["output"].as_str().unwrap());
        assert!(output.is_absolute());
        assert_eq!(hash_file(output).unwrap(), row["sha256"]);
        assert!(row["outputBytes"].as_u64().unwrap() <= row["originalBytes"].as_u64().unwrap());
        if row["source"] == sixteen.to_str().unwrap() && row["mode"] == "smaller" {
            assert_eq!(row["disposition"], "compressed");
            assert_eq!(row["settings"]["lossless"], true);
        }
        if row["source"] == wav.to_str().unwrap()
            && row["mode"] == "lossless"
            && row["disposition"] == "compressed"
        {
            assert_eq!(row["diagnostics"]["samplesIdentical"], true);
        }
        if row["mode"] == "lossless"
            && row["disposition"] == "compressed"
            && row["source"] != wav.to_str().unwrap()
        {
            assert_eq!(row["diagnostics"]["pixelIdentical"], true);
        }
    }
    // When Both has already generated an exact candidate, Smaller must not choose
    // a larger lossy result. No extra study/encoding is required for this decision.
    for smaller in result["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["mode"] == "smaller")
    {
        let lossless = result["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["mode"] == "lossless" && r["source"] == smaller["source"])
            .unwrap();
        assert!(
            smaller["outputBytes"].as_u64().unwrap() <= lossless["outputBytes"].as_u64().unwrap()
        );
    }
    assert!(result["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["output"].as_str().unwrap().contains("nested")));
    let cache_times: Vec<_> = std::fs::read_dir(workspace.join("cache"))
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (e.path(), e.metadata().unwrap().modified().unwrap())
        })
        .collect();
    let (code, again) = compress_folder(root, &workspace, &folder, "both");
    assert_eq!(code, 2, "{again}");
    assert_eq!(again["sourceFiles"], 5);
    assert_eq!(again["plannedEncodes"], 7);
    for (p, t) in cache_times {
        assert_eq!(
            std::fs::metadata(&p).unwrap().modified().unwrap(),
            t,
            "Verified cache should avoid regenerating outputs: {p:?}"
        );
    }
    for (p, h) in originals {
        assert_eq!(hash_file(&p).unwrap(), h);
    }
    for row in again["outputs"].as_array().unwrap() {
        assert!(!result["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["output"] == row["output"]));
    }
    eprintln!(
        "Generated mixed-folder timings: first {}s; verified cache {}s",
        result["elapsedSeconds"], again["elapsedSeconds"]
    );
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn automatic_folder_mcp_is_one_call_and_rejects_ungranted_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("images");
    std::fs::create_dir(&folder).unwrap();
    let source = fixture(&folder);
    let original = hash_file(&source).unwrap();
    let mut client = Client::new(root, &root.join("cache"));
    let denied = client.call(
        "compress_folder",
        json!({"path":folder,"destination":root.parent().unwrap().join("outside-grant")}),
    );
    assert_eq!(denied["isError"], true);
    assert!(!folder.join("Lossless").exists());
    let result = client.call("compress_folder", json!({"path":folder}));
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(result["structuredContent"]["mode"], "lossless");
    assert_eq!(result["structuredContent"]["sourceFiles"], 1);
    assert_eq!(result["structuredContent"]["plannedEncodes"], 1);
    let again = client.call("compress_folder", json!({"path":folder,"mode":"both"}));
    assert_eq!(again["isError"], false, "{again}");
    assert_eq!(again["structuredContent"]["sourceFiles"], 1);
    assert_eq!(
        again["structuredContent"]["outputs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(hash_file(&source).unwrap(), original);
    let compact = root.join("already-small");
    std::fs::create_dir(&compact).unwrap();
    let first = &result["structuredContent"]["outputs"][0];
    assert_eq!(first["disposition"], "compressed");
    let compact_source = compact.join("original.webp");
    std::fs::copy(first["output"].as_str().unwrap(), &compact_source).unwrap();
    let kept = client.call("compress_folder", json!({"path":compact}));
    assert_eq!(kept["isError"], false, "{kept}");
    assert_eq!(
        kept["structuredContent"]["summaries"][0]["keptOriginalFiles"],
        1
    );
    assert_eq!(
        kept["structuredContent"]["outputs"][0]["sha256"],
        hash_file(&compact_source).unwrap()
    );
    client.close();
}

#[test]
#[ignore = "requires bundled native codecs/worker"]
fn automatic_folder_cancels_during_import_and_blocks_interfering_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("images");
    std::fs::create_dir(&folder).unwrap();
    let source = fixture(&folder);
    for n in 0..128 {
        std::fs::copy(&source, folder.join(format!("image-{n}.png"))).unwrap();
    }
    let hash = hash_file(&source).unwrap();
    let mut client = Client::new(root, &root.join("cache"));
    client.id += 1;
    let id = client.id;
    client.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"compress_folder","arguments":{"path":folder,"mode":"both"}}}));
    let start = Instant::now();
    loop {
        let status = client.call("job_status", json!({}));
        if status["structuredContent"]["active"] == true {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    let interfering = client.call("import_media", json!({"paths":[source]}));
    assert_eq!(interfering["isError"], true, "{interfering}");
    let cancel = Instant::now();
    assert_eq!(client.call("cancel_study", json!({}))["isError"], false);
    let response = client.response(id);
    assert!(cancel.elapsed() < Duration::from_secs(5));
    assert!(
        response["result"]["isError"] == true
            || response["result"]["structuredContent"]["canceled"] == true,
        "{response}"
    );
    assert_eq!(
        client.call("job_status", json!({}))["structuredContent"]["active"],
        false
    );
    assert!(!folder.join("Lossless").exists());
    assert!(!folder.join("Smaller").exists());
    assert_eq!(hash_file(&source).unwrap(), hash);
    client.close();
}

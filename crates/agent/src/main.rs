use anyhow::{bail, Context, Result};
use media_agent::{
    protocol::{tools, AgentServer},
    session::{bundled_tools, has_errors, read_json, Scope, Session, MAX_JSON_BYTES},
};
use media_engine::Settings;
use rmcp::{transport::stdio, ServiceExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{ffi::OsString, io::Read, path::PathBuf, sync::Arc};

const HELP: &str = r#"Media Compression agent interface — MCP stdio + JSON CLI

  media-compression-agent mcp --root <folder> [--root <folder> ...]
  media-compression-agent config --root <folder>
  media-compression-agent capabilities
  media-compression-agent tools
  media-compression-agent guide
  media-compression-agent inspect <file-or-folder> ... --root <folder>
  media-compression-agent study --input <request.json|-> --manifest <study.json> --root <folder>
  media-compression-agent export --input <request.json|-> --root <folder>

Every data command returns schema-versioned JSON on stdout. MCP reserves stdout
for protocol messages. --help is human-readable; errors are JSON on stderr.
--workspace <empty-or-owned-folder> chooses a separate locked agent cache;
the default is the platform cache's media-compression/agent-v1 directory.
MCP requires explicit --root grants. CLI defaults to the current directory.
Exit codes: 0 success, 1 invalid request/runtime failure, 2 partial file failure,
130 canceled. Processing is local; no model/account/API key is bundled.

Study JSON: {"paths":["/media/image.png"],"settings":[
 {"format":"webp","lossless":false,"quality":80},
 {"format":"webp","lossless":false,"quality":60}]}
Export JSON: {"study":"/media/study.json","candidateIds":["ID"],
 "destination":"/media/export","report":true}
See tools for MCP schemas and the bundled skill for study/refinement guidance.
"#;

#[derive(Clone)]
struct Options {
    command: String,
    roots: Vec<PathBuf>,
    workspace: Option<PathBuf>,
    input: Option<PathBuf>,
    manifest: Option<PathBuf>,
    paths: Vec<PathBuf>,
}
impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut args = std::env::args_os().skip(1);
        let command = args.next().unwrap_or_else(|| OsString::from("--help"));
        if command == "--help" || command == "-h" {
            print!("{HELP}");
            return Ok(None);
        }
        if command == "--version" {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        let mut opts = Self {
            command: command
                .into_string()
                .map_err(|_| anyhow::anyhow!("Command is not UTF-8"))?,
            roots: Vec::new(),
            workspace: None,
            input: None,
            manifest: None,
            paths: Vec::new(),
        };
        if ![
            "mcp",
            "config",
            "capabilities",
            "tools",
            "guide",
            "inspect",
            "study",
            "export",
        ]
        .contains(&opts.command.as_str())
        {
            bail!("Unknown command; use --help");
        }
        let mut positional = false;
        while let Some(arg) = args.next() {
            if !positional && arg == "--" {
                positional = true;
                continue;
            }
            if !positional && arg == "--help" {
                print!("{HELP}");
                return Ok(None);
            }
            let flag = arg.to_str().unwrap_or("");
            if !positional && ["--root", "--workspace", "--input", "--manifest"].contains(&flag) {
                let value = PathBuf::from(args.next().context("Option needs a value")?);
                match flag {
                    "--root" => opts.roots.push(value),
                    "--workspace" => opts.workspace = Some(value),
                    "--input" => opts.input = Some(value),
                    _ => opts.manifest = Some(value),
                }
            } else if !positional && flag.starts_with('-') {
                bail!("Unknown option {flag}; use --help");
            } else {
                opts.paths.push(PathBuf::from(arg));
            }
        }
        if opts.command != "inspect" && !opts.paths.is_empty() {
            bail!("Unexpected positional input; use --help");
        }
        if ["study", "export"].contains(&opts.command.as_str()) && opts.input.is_none() {
            bail!("Provide --input <JSON file or ->");
        }
        if opts.command == "study" && opts.manifest.is_none() {
            bail!("Provide --manifest <new study.json>");
        }
        if ["mcp", "config"].contains(&opts.command.as_str()) && opts.roots.is_empty() {
            bail!("Grant a folder with --root <folder>");
        }
        if opts.roots.is_empty() {
            opts.roots.push(std::env::current_dir()?);
        }
        Ok(Some(opts))
    }
    fn request(&self) -> Result<Value> {
        let p = self.input.as_ref().context("Input missing")?;
        if p.as_os_str() == "-" {
            let mut data = Vec::new();
            std::io::stdin()
                .take(MAX_JSON_BYTES + 1)
                .read_to_end(&mut data)?;
            if data.len() as u64 > MAX_JSON_BYTES {
                bail!("JSON input exceeds 64 MiB");
            }
            Ok(serde_json::from_slice(&data)?)
        } else {
            read_json(p)
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StudyRequest {
    paths: Vec<PathBuf>,
    settings: Vec<Settings>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportRequest {
    study: PathBuf,
    candidate_ids: Vec<String>,
    destination: PathBuf,
    #[serde(default)]
    report: bool,
}

async fn run_cli(session: Arc<Session>, opts: Options) -> Result<Value> {
    match opts.command.as_str() {
        "inspect" => {
            let s = session.clone();
            tokio::task::spawn_blocking(move || s.import(opts.paths)).await??;
            Ok(session.snapshot())
        }
        "study" => {
            let request: StudyRequest = serde_json::from_value(opts.request()?)?;
            let manifest = opts.manifest.as_ref().unwrap();
            let target = session.scope.path(manifest)?;
            if target.exists() {
                bail!("Study manifest already exists; choose another filename");
            }
            let s = session.clone();
            tokio::task::spawn_blocking(move || s.import(request.paths)).await??;
            let items = session
                .engine
                .snapshot()
                .media
                .iter()
                .filter(|m| m.error.is_none())
                .map(|m| (m.id.clone(), request.settings.clone()))
                .collect();
            if session.engine.snapshot().media.is_empty() {
                bail!("No files found in this selection");
            }
            session.engine.start(items)?;
            session.wait().await;
            let saved = session.save(manifest)?;
            let mut result = session.snapshot();
            result["manifest"] = saved["manifest"].clone();
            Ok(result)
        }
        "export" => {
            let args: ExportRequest = serde_json::from_value(opts.request()?)?;
            session
                .restore_export(
                    &args.study,
                    args.candidate_ids,
                    &args.destination,
                    args.report,
                )
                .await
        }
        _ => bail!("Unsupported data command"),
    }
}

struct Shutdown;
impl Drop for Shutdown {
    fn drop(&mut self) {
        media_engine::process::shutdown();
    }
}

#[tokio::main(worker_threads = 2)]
async fn main() {
    let result = execute().await;
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!(
                "{}",
                json!({"schemaVersion":1,"error":{"code":"request_or_runtime_error","message":format!("{error:#}")}})
            );
            std::process::exit(1);
        }
    }
}
async fn execute() -> Result<i32> {
    let Some(opts) = Options::parse()? else {
        return Ok(0);
    };
    if opts.command == "capabilities" {
        println!("{}", media_engine::settings::capabilities());
        return Ok(0);
    }
    if opts.command == "tools" {
        println!("{}", json!({"schemaVersion":1,"tools":tools()}));
        return Ok(0);
    }
    if opts.command == "guide" {
        println!(
            "{}",
            json!({"schemaVersion":1,"instructions":include_str!("../../../agent-plugin/skills/media-compression/SKILL.md")})
        );
        return Ok(0);
    }
    let scope = Scope::new(opts.roots.clone())?;
    if opts.command == "config" {
        println!(
            "{}",
            json!({"mcpServers":{"media-compression":{"command":std::env::current_exe()?,"args":std::iter::once("mcp".to_owned()).chain(scope.roots.iter().flat_map(|r|vec!["--root".to_owned(),r.to_string_lossy().into_owned()])).collect::<Vec<_>>()}}})
        );
        return Ok(0);
    }
    let workspace = opts.workspace.clone().unwrap_or(
        dirs::cache_dir()
            .context("Platform cache folder unavailable; provide --workspace")?
            .join("media-compression/agent-v1"),
    );
    let session = Arc::new(Session::new(workspace, scope, bundled_tools()?)?);
    let _shutdown = Shutdown;
    if opts.command == "mcp" {
        let service = AgentServer::new(session.clone()).serve(stdio()).await?;
        let cancel = service.cancellation_token();
        tokio::select! {
            result=service.waiting()=>{result?;}
            signal=tokio::signal::ctrl_c()=>{signal?;session.engine.cancel(None);cancel.cancel();}
        }
        session.engine.cancel(None);
        return Ok(0);
    }
    let execution = run_cli(session.clone(), opts.clone());
    tokio::pin!(execution);
    let (mut result, canceled) = tokio::select! {
        value=&mut execution=>(value?,false),
        signal=tokio::signal::ctrl_c()=>{
            signal?;session.engine.cancel(None);media_engine::process::shutdown();session.wait().await;
            let mut value=session.snapshot();value["canceled"]=json!(true);
            if opts.command=="study" {if let Some(p)=&opts.manifest {if let Ok(saved)=session.save(p){value["manifest"]=saved["manifest"].clone();}}}
            (value,true)
        }
    };
    result["applicationVersion"] = json!(env!("CARGO_PKG_VERSION"));
    let code = if canceled {
        130
    } else if has_errors(&result) {
        2
    } else {
        0
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(code)
}

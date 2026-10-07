use crate::session::Session;
use anyhow::{bail, Context, Result};
use base64::Engine as _;
use media_engine::Settings;
use rmcp::{
    model::*,
    service::{RequestContext, RoleServer},
    ServerHandler,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct AgentServer {
    pub session: Arc<Session>,
    preview_lock: Arc<Mutex<()>>,
}
impl AgentServer {
    pub fn new(session: Arc<Session>) -> Self {
        Self {
            session,
            preview_lock: Arc::new(Mutex::new(())),
        }
    }
    pub fn dispatch(&self, name: &str, input: Value) -> Result<CallToolResult> {
        let session = &self.session;
        let value = match name {
            "compression_capabilities" => {
                let _: Empty = serde_json::from_value(input)?;
                let mut v = media_engine::settings::capabilities();
                v["grantedFolders"] = json!(session.scope.roots);
                v["qualityNotice"] = json!("Quality controls and diagnostics do not guarantee perceptual equivalence. Every encode starts from the original. Larger and aggressive candidates may be exported explicitly.");
                v
            }
            "import_media" => {
                let args: Paths = serde_json::from_value(input)?;
                session.import(args.paths)?;
                media_page(session.snapshot(), 0, 50)?
            }
            "list_media" => {
                let args: Page = serde_json::from_value(input)?;
                media_page(session.snapshot(), args.offset, args.limit)?
            }
            "plan_study" | "start_study" => {
                let args: Study = serde_json::from_value(input)?;
                if args.items.is_empty() {
                    bail!("Provide at least one study item");
                }
                let encodes: usize = args.items.iter().map(|i| i.settings.len()).sum();
                let snapshot = session.engine.snapshot();
                let mut conversions = Vec::new();
                let mut eligible = 0usize;
                for item in &args.items {
                    if item.settings.is_empty() || item.settings.len() > 512 {
                        bail!("Provide 1–512 settings per study item");
                    }
                    let media = snapshot
                        .media
                        .iter()
                        .find(|m| m.id == item.media_id)
                        .context("Unknown media ID")?;
                    if let Some(p) = media.properties.as_ref().filter(|_| media.error.is_none()) {
                        for s in &item.settings {
                            media_engine::settings::validate(s, p)?;
                            let rate = media_engine::settings::output_rate(s, p);
                            if p.kind == "audio" && p.sample_rate != Some(rate) {
                                conversions.push(json!({"mediaId":item.media_id,"format":s.format,"sourceRate":p.sample_rate,"outputRate":rate}));
                            }
                        }
                        eligible += item.settings.len();
                    }
                }
                if name == "plan_study" {
                    return Ok(CallToolResult::structured(
                        json!({"schemaVersion":1,"plannedEncodes":encodes,"eligibleEncodes":eligible,"maxWorkers":2,"sampleRateConversions":conversions,"notice":"This validates settings without starting jobs; exact sizes and quality require real encodes."}),
                    ));
                }
                let ids = session.engine.start(
                    args.items
                        .into_iter()
                        .map(|i| (i.media_id, i.settings))
                        .collect(),
                )?;
                json!({"schemaVersion":1,"jobIds":ids,"plannedEncodes":encodes,"sampleRateConversions":conversions,"next":"Poll job_status; inspect each media with get_candidates after completion. Partial failures retain completed candidates."})
            }
            "job_status" => {
                let args: JobIds = serde_json::from_value(input)?;
                let snapshot = session.engine.snapshot();
                for id in &args.job_ids {
                    if !snapshot.jobs.iter().any(|j| &j.id == id) {
                        bail!("Unknown job ID");
                    }
                }
                let jobs: Vec<_> = snapshot
                    .jobs
                    .into_iter()
                    .filter(|j| args.job_ids.is_empty() || args.job_ids.contains(&j.id))
                    .collect();
                json!({"schemaVersion":1,"active":session.active(),"jobs":jobs})
            }
            "get_candidates" => {
                let args: MediaId = serde_json::from_value(input)?;
                let snapshot = session.snapshot();
                let media = snapshot["media"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m["id"] == args.media_id)
                    .context("Unknown media ID")?;
                json!({"schemaVersion":1,"media":media,"codecVersions":snapshot["tools"],"notice":"Savings refer to bytes. SSIM/alpha/timing scores are diagnostics, not perceived-quality percentages."})
            }
            "cancel_study" => {
                let args: Cancel = serde_json::from_value(input)?;
                if let Some(id) = &args.job_id {
                    if !session.engine.snapshot().jobs.iter().any(|j| &j.id == id) {
                        bail!("Unknown job ID");
                    }
                }
                session.engine.cancel(args.job_id.as_deref());
                json!({"schemaVersion":1,"cancellationRequested":true,"next":"Poll job_status until queued/processing work ends. Completed candidates remain available."})
            }
            "export_candidates" => {
                let args: Export = serde_json::from_value(input)?;
                session.export(args.candidate_ids, &args.destination, args.report)?
            }
            "save_study" => {
                let args: Save = serde_json::from_value(input)?;
                session.save(&args.path)?
            }
            "preview_image" => {
                let args: Preview = serde_json::from_value(input)?;
                let snapshot = session.engine.snapshot();
                let source = snapshot
                    .media
                    .iter()
                    .find(|m| m.id == args.id || m.candidates.iter().any(|c| c.id == args.id))
                    .context("Unknown image ID")?;
                session.scope.path(&source.path)?;
                let _guard = self.preview_lock.lock().unwrap();
                let path = session.engine.preview(&args.id)?;
                let image = image::open(path)?.thumbnail(1024, 1024);
                let width = image.width();
                let height = image.height();
                let mut out = Cursor::new(Vec::new());
                image.write_to(&mut out, image::ImageFormat::Png)?;
                let mut result = CallToolResult::structured(
                    json!({"schemaVersion":1,"id":args.id,"previewWidth":width,"previewHeight":height,"thumbnail":true,"notice":"Color-managed preview only; exported dimensions are unchanged. Audio requires listening and supported diagnostics."}),
                );
                result.content.push(ContentBlock::image(
                    base64::engine::general_purpose::STANDARD.encode(out.into_inner()),
                    "image/png",
                ));
                return Ok(result);
            }
            _ => bail!("Unknown tool: {name}"),
        };
        let failed = name == "export_candidates" && !value["exportError"].is_null();
        let mut result = CallToolResult::structured(value);
        if failed {
            result.is_error = Some(true);
        }
        Ok(result)
    }
}

fn media_page(mut snapshot: Value, offset: usize, limit: usize) -> Result<Value> {
    if limit == 0 || limit > 100 {
        bail!("Page limit must be 1–100");
    }
    let all = snapshot["media"].as_array_mut().unwrap();
    let count = all.len();
    let rows: Vec<_> = all
        .iter()
        .skip(offset)
        .take(limit)
        .map(|m| {
            let mut m = m.clone();
            m["candidateCount"] = json!(m["candidates"].as_array().map_or(0, Vec::len));
            m.as_object_mut().unwrap().remove("candidates");
            m
        })
        .collect();
    Ok(
        json!({"schemaVersion":1,"total":count,"offset":offset,"nextOffset":if offset.saturating_add(limit)<count {Some(offset+limit)} else {None},"media":rows}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paths {
    paths: Vec<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
}
fn page_size() -> usize {
    50
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Item {
    media_id: String,
    settings: Vec<Settings>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Study {
    items: Vec<Item>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobIds {
    #[serde(default)]
    job_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaId {
    media_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cancel {
    #[serde(default)]
    job_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Export {
    candidate_ids: Vec<String>,
    destination: PathBuf,
    #[serde(default)]
    report: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preview {
    id: String,
}

pub fn tools() -> Vec<Tool> {
    let setting = json!({"type":"object","additionalProperties":false,"required":["format","lossless"],"properties":{
        "format":{"type":"string","enum":["png","jpeg","webp","aac","mp3","opus","flac"]},"lossless":{"type":"boolean"},
        "quality":{"type":["number","null"],"description":"Image quality 0–100; JPEG/PNG integers. Not a perceptual similarity percentage."},
        "bitrate":{"type":["number","null"],"description":"Audio kbps; limits depend on codec, sample rate and channels."},
        "vbrQuality":{"type":["number","null"],"description":"MP3 VBR 0–9.999; lower is higher quality."},
        "effort":{"type":["integer","null"],"minimum":0,"maximum":12},"background":{"type":["string","null"],"description":"Explicit #RRGGBB for transparent-to-JPEG conversion."}}});
    let array_strings = json!({"type":"array","items":{"type":"string"}});
    let definitions=vec![
        ("compression_capabilities","Inspect supported formats, ranges, codec constraints, worker bounds and granted folders before choosing settings.",json!({}),vec![],true),
        ("import_media","Import files/folders inside granted roots. Originals remain unchanged. Returns the first media page including per-file errors; paginate with list_media.",json!({"paths":array_strings}),vec!["paths"],false),
        ("list_media","Page imported media IDs/properties/errors. Use get_candidates for a particular file's actual results.",json!({"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100}}),vec![],true),
        ("start_study","Encode custom candidates from originals using at most two workers. Returns immediately with job IDs and planned work; poll job_status. Do not treat quality as a percentage or promise a ratio.",json!({"items":{"type":"array","minItems":1,"items":{"type":"object","additionalProperties":false,"required":["mediaId","settings"],"properties":{"mediaId":{"type":"string"},"settings":{"type":"array","minItems":1,"maxItems":512,"items":setting}}}}}),vec!["items"],false),
        ("job_status","Read job states, progress and errors; completed candidates survive partial failure or cancellation.",json!({"jobIds":array_strings}),vec![],true),
        ("get_candidates","Read a file's candidate IDs, producing settings, exact bytes/savings, hashes, properties and diagnostics. Larger/aggressive outputs remain valid choices.",json!({"mediaId":{"type":"string"}}),vec!["mediaId"],true),
        ("preview_image","Get a color-managed PNG thumbnail of an original or candidate for visual inspection. Thumbnail size does not change the exported image. Audio quality needs listening.",json!({"id":{"type":"string"}}),vec!["id"],false),
        ("cancel_study","Cancel one job or all jobs. Queued work cancels immediately; active processes terminate. Poll status to confirm completion.",json!({"jobId":{"type":["string","null"]}}),vec![],false),
        ("export_candidates","Export explicitly selected candidate IDs into a granted folder. Verify source/output integrity, preserve folder structure, number collisions, optionally write a JSON report. Partial errors are returned alongside successful exports.",json!({"candidateIds":array_strings,"destination":{"type":"string"},"report":{"type":"boolean"}}),vec!["candidateIds","destination"],false),
        ("save_study","Save a completed or canceled study manifest without overwriting. Contains private local source paths. The CLI can verify/reopen selected candidates for later export; cached content is never trusted blindly.",json!({"path":{"type":"string"}}),vec!["path"],false),
    ];
    let mut result:Vec<Tool>=definitions.into_iter().map(|(name,description,properties,required,read_only)| {
        let schema=json!({"type":"object","additionalProperties":false,"properties":properties,"required":required});
        Tool::new(name,description,schema.as_object().unwrap().clone()).with_annotations(ToolAnnotations::new().read_only(read_only).destructive(false).open_world(false))
    }).collect();
    let mut plan = result
        .iter()
        .find(|t| t.name == "start_study")
        .unwrap()
        .clone();
    plan.name = "plan_study".into();
    plan.description=Some("Validate a proposed study and disclose work/sample-rate conversions before encoding. Does not start jobs; output size and perceptual quality cannot be predicted exactly.".into());
    plan.annotations = Some(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(false),
    );
    result.push(plan);
    result
}

impl ServerHandler for AgentServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions =
            Some(include_str!("../../../agent-plugin/skills/media-compression/SKILL.md").into());
        info.server_info.name = "media-compression".into();
        info.server_info.version = env!("CARGO_PKG_VERSION").into();
        info
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|t| t.name == name)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let handler = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            handler.dispatch(
                &request.name,
                Value::Object(request.arguments.unwrap_or_default()),
            )
        })
        .await;
        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => CallToolResult::error(vec![ContentBlock::text(format!("{e:#}"))]),
            Err(e) => {
                CallToolResult::error(vec![ContentBlock::text(format!("Tool task failed: {e}"))])
            }
        };
        Ok(output.into())
    }
}

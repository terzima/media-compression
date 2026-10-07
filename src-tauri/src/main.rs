#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod playback;
use media_engine::{Engine, Settings, Snapshot, Tools};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

struct AppState {
    engine: Result<Arc<Engine>, String>,
    playback: playback::Playback,
    play_request: AtomicU64,
    play_cancel: Mutex<Arc<AtomicBool>>,
}
fn engine(state: &AppState) -> Result<Arc<Engine>, String> {
    state.engine.clone()
}
fn interrupt_playback(
    state: &AppState,
    action: &str,
    value: Option<f64>,
    end: Option<f64>,
) -> Result<(), String> {
    let cancel = state.play_cancel.lock().unwrap();
    state.play_request.fetch_add(1, Ordering::SeqCst);
    cancel.store(true, Ordering::SeqCst);
    state.playback.control(action, value, end)
}
#[tauri::command]
fn encoder_capabilities() -> serde_json::Value {
    serde_json::json!({"schemaVersion":1,"image":{"jpeg":{"lossless":false,"quality":[0,100],"integerQuality":true},"webp":{"lossless":true,"quality":[0,100],"integerQuality":false,"effort":[0,6]},"png":{"lossless":true,"quality":[0,100],"integerQuality":true,"losslessEffort":[0,6],"paletteSpeed":[1,11]}},"audio":{"aac":{"bitrateKbps":[0.001,1152],"maximumDependsOnRateAndChannels":true,"sampleRates":[7350,8000,11025,12000,16000,22050,24000,32000,44100,48000,64000,88200,96000]},"mp3":{"vbrQuality":[0,9.999],"bitratesKbps":[8,16,24,32,40,48,56,64,80,96,112,128,144,160,192,224,256,320],"effort":[0,9]},"opus":{"bitrateKbps":[0.5,512],"maximumKbpsPerChannel":256,"sampleRate":48000,"effort":[0,10]},"flac":{"lossless":true,"integerBitDepths":[16,24],"effort":[0,12]}},"maxWorkers":2,"maxStudySettings":512,"maxImagePixels":50000000,"notice":"Rate, channel and codec combinations are validated; a valid extreme setting may still fail for a particular source."})
}
#[tauri::command]
fn snapshot(state: State<AppState>) -> Result<Snapshot, String> {
    Ok(engine(&state)?.snapshot())
}
#[tauri::command]
async fn add_files(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    folder: bool,
) -> Result<(), String> {
    let engine = engine(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let paths = if folder {
            app.dialog().file().blocking_pick_folder().map(|p| vec![p])
        } else {
            app.dialog().file().blocking_pick_files()
        };
        let Some(paths) = paths else { return Ok(()) };
        let paths = paths
            .into_iter()
            .map(|p| p.into_path().map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        engine.import(paths).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn add_paths(state: State<'_, AppState>, paths: Vec<PathBuf>) -> Result<(), String> {
    let engine = engine(&state)?;
    tauri::async_runtime::spawn_blocking(move || engine.import(paths).map_err(|e| format!("{e:#}")))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn image_preview(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let engine = engine(&state)?;
    tauri::async_runtime::spawn_blocking(move || engine.preview(&id).map_err(|e| format!("{e:#}")))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn start_jobs(
    state: State<AppState>,
    items: Vec<(String, Vec<Settings>)>,
) -> Result<Vec<String>, String> {
    engine(&state)?.start(items).map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn cancel_jobs(state: State<AppState>, id: Option<String>) -> Result<(), String> {
    engine(&state)?.cancel(id.as_deref());
    Ok(())
}
#[tauri::command]
async fn export_candidates(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
    report: bool,
) -> Result<Vec<String>, String> {
    let engine = engine(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(Vec::new());
        };
        let path = folder.into_path().map_err(|e| e.to_string())?;
        engine
            .export(&ids, &path, report)
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn clear_cache(state: State<AppState>) -> Result<bool, String> {
    interrupt_playback(&state, "stop", None, None)?;
    engine(&state)?
        .clear()
        .map(|_| true)
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn playback_info(state: State<AppState>) -> playback::Info {
    state.playback.info()
}
#[tauri::command]
fn playback_control(
    state: State<AppState>,
    action: String,
    value: Option<f64>,
    end: Option<f64>,
) -> Result<(), String> {
    if ["stop", "pause"].contains(&action.as_str()) {
        return interrupt_playback(&state, &action, value, end);
    }
    state.playback.control(&action, value, end)
}
#[tauri::command]
async fn play_media(
    state: State<'_, AppState>,
    id: String,
    position: Option<f64>,
) -> Result<(), String> {
    if position.is_some_and(|v| !v.is_finite() || v < 0.) {
        return Err("Playback position is invalid".into());
    }
    let engine = engine(&state)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let request = {
        let mut previous = state.play_cancel.lock().unwrap();
        previous.store(true, Ordering::SeqCst);
        *previous = cancel.clone();
        state.play_request.fetch_add(1, Ordering::SeqCst) + 1
    };
    let preparation_engine = engine.clone();
    let key = id.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        preparation_engine
            .playback_pcm_cancelable(&key, &cancel)
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?;
    let _preparation = state.play_cancel.lock().unwrap();
    if state.play_request.load(Ordering::SeqCst) != request {
        return Ok(());
    }
    let (path, rate, channels) = prepared?;
    let current = state.playback.info();
    let position = position.unwrap_or_else(|| {
        if current
            .id
            .as_deref()
            .is_some_and(|old| engine.same_media(old, &id))
        {
            current.position
        } else {
            0.
        }
    });
    state.playback.play(id, path, rate, channels, position);
    Ok(())
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let root = app.path().app_cache_dir()?.join("workspace-v1");
            app.asset_protocol_scope().allow_directory(&root, true)?;
            #[cfg(debug_assertions)]
            let codecs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/codecs");
            #[cfg(not(debug_assertions))]
            let codecs = app.path().resource_dir()?.join("resources/codecs");
            let ext = if cfg!(windows) { ".exe" } else { "" };
            #[cfg(debug_assertions)]
            let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                "binaries/media-worker-{}{ext}",
                env!("MEDIA_TARGET")
            ));
            #[cfg(not(debug_assertions))]
            let worker = std::env::current_exe()?
                .parent()
                .unwrap()
                .join(format!("media-worker{ext}"));
            let tools = Tools {
                worker,
                ffmpeg: codecs.join(format!("ffmpeg{ext}")),
                ffprobe: codecs.join(format!("ffprobe{ext}")),
                pngquant: codecs.join(format!("pngquant{ext}")),
                cwebp: codecs.join(format!("cwebp{ext}")),
                cjpeg: codecs.join(format!("cjpeg{ext}")),
            };
            let engine = Engine::new(root, tools, move || {
                let _ = handle.emit("engine-changed", ());
            })
            .map_err(|e| format!("{e:#}"));
            app.manage(AppState {
                engine,
                playback: playback::Playback::new(),
                play_request: AtomicU64::new(0),
                play_cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            encoder_capabilities,
            image_preview,
            snapshot,
            add_files,
            add_paths,
            start_jobs,
            cancel_jobs,
            export_candidates,
            clear_cache,
            play_media,
            playback_control,
            playback_info
        ])
        .build(tauri::generate_context!())
        .expect("Desktop application could not start")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                let state = app.state::<AppState>();
                let _ = interrupt_playback(&state, "stop", None, None);
                if let Ok(engine) = engine(&state) {
                    engine.cancel(None);
                }
                media_engine::process::shutdown();
            }
        });
}

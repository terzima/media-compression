#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod playback;
use media_engine::{Engine, Settings, Snapshot, Tools};
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

struct AppState {
    engine: Result<Arc<Engine>, String>,
    playback: playback::Playback,
}
fn engine(state: &AppState) -> Result<Arc<Engine>, String> {
    state.engine.clone()
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
fn clear_cache(state: State<AppState>) -> Result<(), String> {
    state.playback.control("stop", None, None)?;
    engine(&state)?.clear().map_err(|e| format!("{e:#}"))
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
    state.playback.control(&action, value, end)
}
#[tauri::command]
async fn play_media(state: State<'_, AppState>, id: String, position: f64) -> Result<(), String> {
    if !position.is_finite() || position < 0. {
        return Err("Playback position is invalid".into());
    }
    let engine = engine(&state)?;
    let key = id.clone();
    let (path, rate, channels) = tauri::async_runtime::spawn_blocking(move || {
        engine.playback_pcm(&key).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())??;
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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
        .run(tauri::generate_context!())
        .expect("Desktop application could not start");
}

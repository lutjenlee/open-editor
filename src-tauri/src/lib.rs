pub mod command;
pub mod command_service;
pub mod jobs;
mod media;
mod native;
pub mod project;

use media::{ExportRequest, MediaInspection};
use project::{
    append_history, canonical_folder, create, existing_folder, load, lock_exclusive, save_atomic,
    AnalysisArtifact, ProjectDocument,
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NativePlaybackState {
    value: i64,
    timescale: i32,
    rate: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentProject {
    name: String,
    folder: String,
    opened_at: String,
    pinned: bool,
}

const WHISPER_MODEL_NAME: &str = "ggml-base.en.bin";
const WHISPER_MODEL_SHA1: &str = "137c40403d78fd54d454da0f9bd998f78703390c";
const WHISPER_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

fn whisper_model_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("models/whisper")
        .join(WHISPER_MODEL_NAME))
}

#[tauri::command]
fn transcription_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let model = whisper_model_path(&app)?;
    Ok(serde_json::json!({
        "engineInstalled": media::whisper_tool().is_ok(),
        "modelInstalled": model.is_file(),
        "modelPath": model,
        "modelName": "Whisper base.en",
        "downloadSizeBytes": 148_000_000_u64,
        "license": "MIT model distribution; Whisper model weights",
        "fullyLocalAfterInstall": true
    }))
}

#[tauri::command]
fn start_transcription_model_download(
    app: tauri::AppHandle,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, String> {
    let target = whisper_model_path(&app)?;
    if target.is_file() {
        return Err("The Whisper model is already installed".into());
    }
    let (record, context) = manager.create("modelDownload", None, Some(app))?;
    tauri::async_runtime::spawn_blocking(move || {
        context.running("Downloading Whisper base.en (148 MB)", 0.05);
        if let Some(parent) = target.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                context.fail(error.to_string());
                return;
            }
        }
        let temporary = target.with_extension(format!("{}.download", Uuid::new_v4()));
        let mut child = match std::process::Command::new("/usr/bin/curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--output",
            ])
            .arg(&temporary)
            .arg(WHISPER_MODEL_URL)
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                context.fail(error.to_string());
                return;
            }
        };
        context.register_process(Some(child.id()));
        let status = child.wait();
        context.register_process(None);
        if context.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            context.finish_cancelled();
            return;
        }
        if !status.is_ok_and(|status| status.success()) {
            let _ = std::fs::remove_file(&temporary);
            context.fail("Whisper model download failed");
            return;
        }
        context.running("Verifying Whisper model", 0.92);
        let checksum = std::process::Command::new("/usr/bin/shasum")
            .args(["-a", "1"])
            .arg(&temporary)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .and_then(|line| line.split_whitespace().next().map(str::to_string));
        if checksum.as_deref() != Some(WHISPER_MODEL_SHA1) {
            let _ = std::fs::remove_file(&temporary);
            context.fail("Whisper model checksum did not match the pinned release");
            return;
        }
        if let Err(error) = std::fs::rename(&temporary, &target) {
            let _ = std::fs::remove_file(&temporary);
            context.fail(error.to_string());
            return;
        }
        context.complete(serde_json::json!({ "modelPath": target }));
    });
    Ok(record)
}

#[tauri::command]
fn delete_transcription_model(app: tauri::AppHandle) -> Result<(), String> {
    let target = whisper_model_path(&app)?;
    if target.exists() {
        std::fs::remove_file(target).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn recent_projects_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let folder = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder.join("recent-projects.json"))
}

fn read_recent_projects(app: &tauri::AppHandle) -> Result<Vec<RecentProject>, String> {
    let path = recent_projects_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn write_recent_projects(app: &tauri::AppHandle, items: &[RecentProject]) -> Result<(), String> {
    let target = recent_projects_path(app)?;
    let temporary = target.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(items).map_err(|error| error.to_string())?;
    std::fs::write(&temporary, data).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, target).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_recent_projects(app: tauri::AppHandle) -> Result<Vec<RecentProject>, String> {
    read_recent_projects(&app)
}

#[tauri::command]
fn remember_recent_project(
    app: tauri::AppHandle,
    name: String,
    folder: String,
) -> Result<Vec<RecentProject>, String> {
    let folder = existing_folder(&folder).map_err(|error| error.to_string())?;
    let folder = folder.to_string_lossy().into_owned();
    let mut items = read_recent_projects(&app)?;
    let pinned = items
        .iter()
        .find(|item| item.folder == folder)
        .is_some_and(|item| item.pinned);
    items.retain(|item| item.folder != folder);
    items.insert(
        0,
        RecentProject {
            name,
            folder,
            opened_at: chrono::Utc::now().to_rfc3339(),
            pinned,
        },
    );
    items.truncate(24);
    write_recent_projects(&app, &items)?;
    Ok(items)
}

#[tauri::command]
fn set_recent_project_pinned(
    app: tauri::AppHandle,
    folder: String,
    pinned: bool,
) -> Result<Vec<RecentProject>, String> {
    let mut items = read_recent_projects(&app)?;
    let item = items
        .iter_mut()
        .find(|item| item.folder == folder)
        .ok_or_else(|| "Recent project does not exist".to_string())?;
    item.pinned = pinned;
    write_recent_projects(&app, &items)?;
    Ok(items)
}

#[tauri::command]
fn remove_recent_project(
    app: tauri::AppHandle,
    folder: String,
) -> Result<Vec<RecentProject>, String> {
    let mut items = read_recent_projects(&app)?;
    items.retain(|item| item.folder != folder);
    write_recent_projects(&app, &items)?;
    Ok(items)
}

#[tauri::command]
fn reveal_project(folder: String) -> Result<(), String> {
    let folder = existing_folder(&folder).map_err(|error| error.to_string())?;
    std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(folder)
        .status()
        .map_err(|error| error.to_string())?
        .success()
        .then_some(())
        .ok_or_else(|| "Finder could not reveal the project".into())
}

#[cfg(target_os = "macos")]
fn on_main_thread<T: Send + 'static>(
    window: &tauri::WebviewWindow,
    operation: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = sender.send(operation());
        })
        .map_err(|error| error.to_string())?;
    receiver.recv().map_err(|error| error.to_string())
}

pub fn dispatch_persisted(
    folder: &std::path::Path,
    envelope: command::CommandEnvelope,
) -> Result<command::CommandResult, project::ProjectError> {
    let _lock = lock_exclusive(folder)?;
    let project = load(folder)?;
    let result = command::dispatch(project, &envelope)?;
    save_atomic(folder, &result.project)?;
    append_history(
        folder,
        &serde_json::json!({ "event": "command", "at": chrono::Utc::now().to_rfc3339(), "envelope": envelope, "newProjectRevision": result.new_project_revision, "affectedEntityIds": result.affected_entity_ids, "forwardPatch": result.forward_patch, "inversePatch": result.inverse_patch }),
    )?;
    Ok(result)
}

pub fn dispatch_batch_persisted(
    folder: &std::path::Path,
    envelopes: Vec<command::CommandEnvelope>,
) -> Result<Vec<command::CommandResult>, project::ProjectError> {
    if envelopes.is_empty() {
        return Err(project::ProjectError::Invalid(
            "command batch cannot be empty".into(),
        ));
    }
    let first = &envelopes[0];
    if envelopes.iter().any(|item| {
        item.project_id != first.project_id
            || item.source != first.source
            || item.batch_id != first.batch_id
            || item.conversation_id != first.conversation_id
    }) {
        return Err(project::ProjectError::Invalid(
            "all commands in a batch must share project, source, conversation, and batch identifiers"
                .into(),
        ));
    }
    let _lock = lock_exclusive(folder)?;
    let mut project = load(folder)?;
    let mut results = Vec::with_capacity(envelopes.len());
    for envelope in &envelopes {
        let result = command::dispatch(project, envelope)?;
        project = result.project.clone();
        results.push(result);
    }
    save_atomic(folder, &project)?;
    append_history(
        folder,
        &serde_json::json!({
            "event": "commandBatch",
            "at": chrono::Utc::now().to_rfc3339(),
            "batchId": first.batch_id,
            "source": first.source,
            "conversationId": first.conversation_id,
            "envelopes": envelopes,
            "firstProjectRevision": results.first().map(|item| item.forward_patch.before.revision),
            "newProjectRevision": project.revision,
            "forwardPatch": { "before": results.first().map(|item| &item.forward_patch.before), "after": &project },
            "inversePatch": { "before": &project, "after": results.first().map(|item| &item.forward_patch.before) }
        }),
    )?;
    Ok(results)
}

#[tauri::command]
fn create_project(folder: String, name: String) -> Result<ProjectDocument, project::ProjectError> {
    let folder = canonical_folder(&folder)?;
    let project = ProjectDocument::new(name);
    create(&folder, &project)?;
    Ok(project)
}

#[tauri::command]
fn open_project(folder: String) -> Result<ProjectDocument, project::ProjectError> {
    let folder = existing_folder(&folder)?;
    load(&folder)
}

#[tauri::command]
fn save_project(folder: String, project: ProjectDocument) -> Result<(), project::ProjectError> {
    let folder = canonical_folder(&folder)?;
    let _lock = lock_exclusive(&folder)?;
    if folder.join(project::PROJECT_FILE).exists() {
        let current = load(&folder)?;
        if project.revision < current.revision {
            return Err(project::ProjectError::Invalid(format!(
                "stale save: incoming revision {}, current {}",
                project.revision, current.revision
            )));
        }
        if project.revision == current.revision
            && serde_json::to_value(&project)? != serde_json::to_value(&current)?
        {
            return Err(project::ProjectError::Invalid(
                "conflicting project snapshots have the same revision".into(),
            ));
        }
    }
    save_atomic(&folder, &project)
}

#[tauri::command]
fn record_history(folder: String, entry: serde_json::Value) -> Result<(), project::ProjectError> {
    let folder = existing_folder(&folder)?;
    let _lock = lock_exclusive(&folder)?;
    append_history(&folder, &entry)
}

#[tauri::command]
fn dispatch_editor_command(
    folder: String,
    envelope: command::CommandEnvelope,
) -> Result<command::CommandResult, project::ProjectError> {
    let folder = existing_folder(&folder)?;
    dispatch_persisted(&folder, envelope)
}

#[tauri::command]
fn authorize_command_project(
    folder: String,
    app: tauri::AppHandle,
    service: tauri::State<'_, command_service::CommandService>,
) -> Result<command_service::ServiceInfo, project::ProjectError> {
    let folder = existing_folder(&folder)?;
    let project = load(&folder)?;
    let model = whisper_model_path(&app).ok();
    service.authorize(project.id, folder, model)
}

#[tauri::command]
fn deauthorize_command_projects(
    service: tauri::State<'_, command_service::CommandService>,
) -> Result<(), project::ProjectError> {
    service.clear_authorization()
}

#[tauri::command]
fn inspect_media(
    paths: Vec<String>,
    project_folder: String,
) -> Result<Vec<MediaInspection>, media::MediaError> {
    let folder = existing_folder(&project_folder)
        .map_err(|error| media::MediaError::Failed(error.to_string()))?;
    project::initialize_layout(&folder)
        .map_err(|error| media::MediaError::Failed(error.to_string()))?;
    paths
        .iter()
        .map(|path| media::inspect(std::path::Path::new(path), &folder))
        .collect()
}

#[tauri::command]
fn relink_media(
    folder: String,
    asset_id: Uuid,
    inspection: MediaInspection,
) -> Result<ProjectDocument, project::ProjectError> {
    let folder = existing_folder(&folder)?;
    let _lock = lock_exclusive(&folder)?;
    let mut project = load(&folder)?;
    let asset = project
        .media
        .iter_mut()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| project::ProjectError::Invalid("media asset does not exist".into()))?;
    if asset.kind != inspection.kind {
        return Err(project::ProjectError::Invalid(format!(
            "replacement must be {} media",
            asset.kind
        )));
    }
    asset.path = inspection.path;
    asset.name = inspection.name;
    asset.duration = inspection.duration;
    asset.width = inspection.width;
    asset.height = inspection.height;
    asset.codec = inspection.codec;
    asset.has_audio = Some(inspection.has_audio);
    asset.thumbnail_path = inspection.thumbnail_path;
    asset.waveform_path = inspection.waveform_path;
    asset.bookmark = inspection.bookmark;
    asset.proxy_path = None;
    asset.status = "ready".into();
    project
        .analysis_artifacts
        .retain(|artifact| artifact.asset_id != asset_id);
    project.revision += 1;
    project.updated_at = chrono::Utc::now().to_rfc3339();
    save_atomic(&folder, &project)?;
    append_history(
        &folder,
        &serde_json::json!({
            "event": "mediaRelinked", "at": project.updated_at, "assetId": asset_id,
            "newProjectRevision": project.revision
        }),
    )?;
    Ok(project)
}

#[tauri::command]
async fn export_video(request: ExportRequest) -> Result<String, media::MediaError> {
    tauri::async_runtime::spawn_blocking(move || media::export(request, None))
        .await
        .map_err(|error| media::MediaError::Failed(error.to_string()))?
}

#[tauri::command]
fn start_export_job(
    request: ExportRequest,
    app: tauri::AppHandle,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, media::MediaError> {
    if request.clips.is_empty() {
        return Err(media::MediaError::Failed(
            "Add at least one video clip before exporting.".into(),
        ));
    }
    let (record, context) = manager
        .create("export", None, Some(app))
        .map_err(media::MediaError::Failed)?;
    tauri::async_runtime::spawn_blocking(move || match media::export(request, Some(&context)) {
        Ok(_path) if context.is_cancelled() => context.finish_cancelled(),
        Ok(path) => context.complete(serde_json::Value::String(path)),
        Err(media::MediaError::Cancelled) => context.finish_cancelled(),
        Err(error) => context.fail(error.to_string()),
    });
    Ok(record)
}

#[cfg(target_os = "macos")]
extern "C" fn native_export_finished(
    success: bool,
    message: *const std::os::raw::c_char,
    context: *mut std::ffi::c_void,
) {
    if context.is_null() {
        return;
    }
    let context = unsafe { Box::from_raw(context as *mut jobs::JobContext) };
    let message = if message.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    if context.is_cancelled() || message == "cancelled" {
        context.finish_cancelled();
    } else if success {
        context.complete(serde_json::Value::String(message));
    } else {
        context.fail(if message.is_empty() {
            "AVFoundation export failed".into()
        } else {
            message
        });
    }
}

#[tauri::command]
fn start_native_export_job(
    folder: String,
    output_path: String,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, String> {
    #[cfg(target_os = "macos")]
    {
        let folder = existing_folder(&folder).map_err(|error| error.to_string())?;
        let project = load(&folder).map_err(|error| error.to_string())?;
        let json = native_composition_json(&folder, &project).map_err(|error| error.to_string())?;
        let (record, context) = manager.create("export", None, Some(app))?;
        context.running("Rendering with AVFoundation", 0.05);
        let callback_context = Box::into_raw(Box::new(context.clone())) as usize;
        let output = output_path.clone();
        let handle = on_main_thread(&window, move || unsafe {
            native::start_export(&json, &output, native_export_finished, callback_context)
        })?;
        if let Some(handle) = handle {
            context.register_native_handle(Some(handle));
            Ok(record)
        } else {
            unsafe { drop(Box::from_raw(callback_context as *mut jobs::JobContext)) };
            context.fail("AVFoundation could not start the export");
            Err("AVFoundation could not start the export".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (folder, output_path, app, window, manager);
        Err("Native export is available only on macOS".into())
    }
}

fn asset_source(
    folder: &std::path::Path,
    project: &ProjectDocument,
    asset_id: Uuid,
) -> Result<std::path::PathBuf, media::MediaError> {
    let asset = project
        .media
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| media::MediaError::Failed("Media asset is not in this project".into()))?;
    if let Some(proxy) = asset.proxy_path.as_deref() {
        let proxy = std::path::PathBuf::from(proxy);
        let proxy = if proxy.is_absolute() {
            proxy
        } else {
            folder.join(proxy)
        };
        if proxy.is_file() {
            return Ok(proxy);
        }
    }
    let path = std::path::PathBuf::from(&asset.path);
    let resolved = asset
        .bookmark
        .as_deref()
        .and_then(native::resolve_security_bookmark)
        .unwrap_or_else(|| {
            if path.is_absolute() {
                path
            } else {
                folder.join(path)
            }
        });
    if !resolved.is_file() {
        return Err(media::MediaError::Failed(
            "Media file is missing; relink it first".into(),
        ));
    }
    Ok(resolved)
}

#[cfg(target_os = "macos")]
fn native_composition_json(
    folder: &std::path::Path,
    project: &ProjectDocument,
) -> Result<String, media::MediaError> {
    let sequence = project
        .sequences
        .iter()
        .find(|sequence| sequence.id == project.active_sequence_id)
        .ok_or_else(|| media::MediaError::Failed("Active sequence does not exist".into()))?;
    let mut clips = Vec::new();
    for track in &sequence.tracks {
        if track.muted || track.kind == "caption" {
            continue;
        }
        for clip in &track.clips {
            let asset = project
                .media
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .ok_or_else(|| media::MediaError::Failed("Timeline media is missing".into()))?;
            let path = asset_source(folder, project, asset.id)?;
            clips.push(serde_json::json!({
                "id": clip.id,
                "path": path,
                "kind": asset.kind,
                "sourceIn": clip.source_in,
                "sourceOut": clip.source_out,
                "timelineStart": clip.timeline_start,
                "playbackRate": clip.playback_rate,
                "transform": clip.transform,
                "audio": clip.audio
            }));
        }
    }
    serde_json::to_string(&serde_json::json!({
        "width": sequence.width,
        "height": sequence.height,
        "frameRate": sequence.frame_rate,
        "clips": clips,
        "captions": sequence.captions,
        "transitions": sequence.transitions
    }))
    .map_err(|error| media::MediaError::Failed(error.to_string()))
}

#[tauri::command]
fn native_preview_attach(
    frame: [f64; 4],
    window: tauri::WebviewWindow,
    player: tauri::State<'_, native::NativePlayer>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if frame[2] <= 0.0 || frame[3] <= 0.0 {
            return Err("Preview frame must be positive".into());
        }
        let view = window.ns_view().map_err(|error| error.to_string())? as usize;
        let handle = player.handle();
        let attached = on_main_thread(&window, move || unsafe {
            native::player_attach(handle, view, frame)
        })?;
        attached
            .then_some(())
            .ok_or_else(|| "Native preview could not attach to the editor window".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (frame, window, player);
        Err("Native preview is available only on macOS".into())
    }
}

#[tauri::command]
fn native_preview_set_frame(
    frame: [f64; 4],
    window: tauri::WebviewWindow,
    player: tauri::State<'_, native::NativePlayer>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let handle = player.handle();
        let changed = on_main_thread(&window, move || unsafe {
            native::player_set_frame(handle, frame)
        })?;
        changed
            .then_some(())
            .ok_or_else(|| "Native preview is not attached".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (frame, window, player);
        Err("Native preview is available only on macOS".into())
    }
}

#[tauri::command]
fn native_preview_load(
    folder: String,
    window: tauri::WebviewWindow,
    player: tauri::State<'_, native::NativePlayer>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let folder = existing_folder(&folder).map_err(|error| error.to_string())?;
        let project = load(&folder).map_err(|error| error.to_string())?;
        let json = native_composition_json(&folder, &project).map_err(|error| error.to_string())?;
        let handle = player.handle();
        let loaded = on_main_thread(&window, move || unsafe {
            native::player_load(handle, &json)
        })?;
        loaded
            .then_some(())
            .ok_or_else(|| "AVFoundation could not build this sequence".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (folder, window, player);
        Err("Native preview is available only on macOS".into())
    }
}

#[tauri::command]
fn native_preview_control(
    action: String,
    value: Option<i64>,
    timescale: Option<i32>,
    window: tauri::WebviewWindow,
    player: tauri::State<'_, native::NativePlayer>,
) -> Result<NativePlaybackState, String> {
    #[cfg(target_os = "macos")]
    {
        let scale = timescale.unwrap_or(600).max(1);
        let handle = player.handle();
        on_main_thread(&window, move || unsafe {
            match action.as_str() {
                "play" => native::player_play(handle),
                "pause" => native::player_pause(handle),
                "seek" => native::player_seek(handle, value.unwrap_or(0), scale),
                "detach" => native::player_detach(handle),
                "status" => {}
                _ => return Err("Unsupported native player action".to_string()),
            }
            let (value, rate) = native::player_time(handle, scale);
            Ok(NativePlaybackState {
                value,
                timescale: scale,
                rate,
            })
        })?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (action, value, timescale, window, player);
        Err("Native preview is available only on macOS".into())
    }
}

pub(crate) fn create_proxy_persisted(
    folder: std::path::PathBuf,
    asset_id: Uuid,
    job: Option<&jobs::JobContext>,
) -> Result<ProjectDocument, media::MediaError> {
    let project = load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let source = asset_source(&folder, &project, asset_id)?;
    let proxy_path = media::create_proxy(&source, &folder, asset_id, job)?;
    let _lock =
        lock_exclusive(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let mut project =
        load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let asset = project
        .media
        .iter_mut()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| media::MediaError::Failed("Media asset is not in this project".into()))?;
    asset.proxy_path = Some(proxy_path);
    project.revision += 1;
    project.updated_at = chrono::Utc::now().to_rfc3339();
    save_atomic(&folder, &project).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    append_history(
        &folder,
        &serde_json::json!({
            "event": "proxyCreated", "at": project.updated_at, "assetId": asset_id,
            "newProjectRevision": project.revision
        }),
    )
    .map_err(|error| media::MediaError::Failed(error.to_string()))?;
    Ok(project)
}

pub(crate) fn analyze_persisted(
    folder: std::path::PathBuf,
    asset_id: Uuid,
    job: Option<&jobs::JobContext>,
) -> Result<ProjectDocument, media::MediaError> {
    let project = load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let source = asset_source(&folder, &project, asset_id)?;
    let report = media::analyze(&source, &folder, asset_id, job)?;
    let _lock =
        lock_exclusive(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let mut project =
        load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();
    project.analysis_artifacts.retain(|artifact| {
        artifact.asset_id != asset_id
            || !matches!(artifact.kind.as_str(), "scenes" | "silence" | "keyframes")
    });
    project.analysis_artifacts.extend([
        AnalysisArtifact {
            id: Uuid::new_v4(),
            asset_id,
            kind: "scenes".into(),
            status: "ready".into(),
            created_at: now.clone(),
            paths: vec![],
            data: serde_json::to_value(report.scene_times).unwrap_or_default(),
        },
        AnalysisArtifact {
            id: Uuid::new_v4(),
            asset_id,
            kind: "silence".into(),
            status: "ready".into(),
            created_at: now.clone(),
            paths: vec![],
            data: serde_json::to_value(report.silence_ranges).unwrap_or_default(),
        },
        AnalysisArtifact {
            id: Uuid::new_v4(),
            asset_id,
            kind: "keyframes".into(),
            status: "ready".into(),
            created_at: now.clone(),
            paths: report.keyframe_paths,
            data: serde_json::Value::Null,
        },
    ]);
    project.revision += 1;
    project.updated_at = now;
    save_atomic(&folder, &project).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    append_history(
        &folder,
        &serde_json::json!({
            "event": "mediaAnalyzed", "at": project.updated_at, "assetId": asset_id,
            "newProjectRevision": project.revision
        }),
    )
    .map_err(|error| media::MediaError::Failed(error.to_string()))?;
    Ok(project)
}

pub(crate) fn transcribe_persisted(
    folder: std::path::PathBuf,
    asset_id: Uuid,
    model: std::path::PathBuf,
    job: Option<&jobs::JobContext>,
) -> Result<ProjectDocument, media::MediaError> {
    let project = load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let asset = project
        .media
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| media::MediaError::Failed("Media asset is not in this project".into()))?;
    if asset.kind == "image" || !asset.has_audio.unwrap_or(asset.kind == "audio") {
        return Err(media::MediaError::Unsupported(
            "Transcription requires video or audio with an audio stream".into(),
        ));
    }
    let source = asset_source(&folder, &project, asset_id)?;
    let (path, data) = media::transcribe(&source, &folder, asset_id, &model, job)?;
    let _lock =
        lock_exclusive(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let mut project =
        load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();
    project
        .analysis_artifacts
        .retain(|artifact| artifact.asset_id != asset_id || artifact.kind != "transcript");
    project.analysis_artifacts.push(AnalysisArtifact {
        id: Uuid::new_v4(),
        asset_id,
        kind: "transcript".into(),
        status: "ready".into(),
        created_at: now.clone(),
        paths: vec![path],
        data,
    });
    project.revision += 1;
    project.updated_at = now;
    save_atomic(&folder, &project).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    append_history(
        &folder,
        &serde_json::json!({
            "event": "mediaTranscribed", "at": project.updated_at, "assetId": asset_id,
            "newProjectRevision": project.revision
        }),
    )
    .map_err(|error| media::MediaError::Failed(error.to_string()))?;
    Ok(project)
}

#[tauri::command]
async fn create_media_proxy(
    folder: String,
    asset_id: Uuid,
) -> Result<ProjectDocument, media::MediaError> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = existing_folder(&folder)
            .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        create_proxy_persisted(folder, asset_id, None)
    })
    .await
    .map_err(|error| media::MediaError::Failed(error.to_string()))?
}

#[tauri::command]
async fn analyze_media_asset(
    folder: String,
    asset_id: Uuid,
) -> Result<ProjectDocument, media::MediaError> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = existing_folder(&folder)
            .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        analyze_persisted(folder, asset_id, None)
    })
    .await
    .map_err(|error| media::MediaError::Failed(error.to_string()))?
}

#[tauri::command]
fn start_media_job(
    folder: String,
    asset_id: Uuid,
    kind: String,
    app: tauri::AppHandle,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, media::MediaError> {
    if !matches!(kind.as_str(), "proxy" | "analysis") {
        return Err(media::MediaError::Failed("Unsupported media job".into()));
    }
    let folder =
        existing_folder(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let project = load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    asset_source(&folder, &project, asset_id)?;
    let (record, context) = manager
        .create(kind.clone(), Some(asset_id), Some(app))
        .map_err(media::MediaError::Failed)?;
    tauri::async_runtime::spawn_blocking(move || {
        context.running(
            if kind == "proxy" {
                "Preparing proxy"
            } else {
                "Preparing analysis"
            },
            0.02,
        );
        let result = if kind == "proxy" {
            create_proxy_persisted(folder, asset_id, Some(&context))
        } else {
            analyze_persisted(folder, asset_id, Some(&context))
        };
        match result {
            Ok(_project) if context.is_cancelled() => context.finish_cancelled(),
            Ok(project) => match serde_json::to_value(project) {
                Ok(value) => context.complete(value),
                Err(error) => context.fail(error.to_string()),
            },
            Err(media::MediaError::Cancelled) => context.finish_cancelled(),
            Err(error) => context.fail(error.to_string()),
        }
    });
    Ok(record)
}

#[tauri::command]
fn start_transcription_job(
    folder: String,
    asset_id: Uuid,
    app: tauri::AppHandle,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, media::MediaError> {
    let model = whisper_model_path(&app).map_err(media::MediaError::Failed)?;
    media::whisper_tool()?;
    if !model.is_file() {
        return Err(media::MediaError::ToolMissing(
            "Whisper base.en model; install it in Preferences".into(),
        ));
    }
    let folder =
        existing_folder(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    let project = load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
    asset_source(&folder, &project, asset_id)?;
    let (record, context) = manager
        .create("transcription", Some(asset_id), Some(app))
        .map_err(media::MediaError::Failed)?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = transcribe_persisted(folder, asset_id, model, Some(&context));
        match result {
            Ok(_) if context.is_cancelled() => context.finish_cancelled(),
            Ok(project) => match serde_json::to_value(project) {
                Ok(value) => context.complete(value),
                Err(error) => context.fail(error.to_string()),
            },
            Err(media::MediaError::Cancelled) => context.finish_cancelled(),
            Err(error) => context.fail(error.to_string()),
        }
    });
    Ok(record)
}

#[tauri::command]
fn get_media_job(
    job_id: Uuid,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, String> {
    manager
        .get(job_id)
        .ok_or_else(|| "job does not exist".into())
}

#[tauri::command]
fn cancel_media_job(
    job_id: Uuid,
    app: tauri::AppHandle,
    manager: tauri::State<'_, jobs::JobManager>,
) -> Result<jobs::JobRecord, String> {
    let record = manager.cancel(job_id)?;
    #[cfg(target_os = "macos")]
    if let Some(handle) = manager.native_handle(job_id) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.run_on_main_thread(move || unsafe {
                native::cancel_export(handle);
            });
        }
    }
    let _ = app.emit("media-job", record.clone());
    Ok(record)
}

#[tauri::command]
fn runtime_capabilities() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": project::CURRENT_SCHEMA_VERSION,
        "minimumMacOS": "14.0",
        "codexTransport": "stdio",
        "ollamaEndpoint": "http://127.0.0.1:11434",
        "nativePreview": cfg!(target_os = "macos")
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(native::NativePlayer::new())
        .manage(command_service::CommandService::default())
        .manage(jobs::JobManager::default())
        .invoke_handler(tauri::generate_handler![
            create_project,
            open_project,
            save_project,
            record_history,
            dispatch_editor_command,
            authorize_command_project,
            deauthorize_command_projects,
            inspect_media,
            relink_media,
            create_media_proxy,
            analyze_media_asset,
            start_media_job,
            start_transcription_job,
            get_media_job,
            cancel_media_job,
            export_video,
            start_export_job,
            start_native_export_job,
            native_preview_attach,
            native_preview_set_frame,
            native_preview_load,
            native_preview_control,
            list_recent_projects,
            remember_recent_project,
            set_recent_project_pinned,
            remove_recent_project,
            reveal_project,
            transcription_status,
            start_transcription_model_download,
            delete_transcription_model,
            runtime_capabilities
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Editor");
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn stale_autosave_cannot_overwrite_a_newer_revision() {
        let root = tempfile::tempdir().unwrap();
        let mut project = ProjectDocument::new("Revision test".into());
        create(root.path(), &project).unwrap();
        project.revision = 2;
        project.updated_at = chrono::Utc::now().to_rfc3339();
        save_project(root.path().display().to_string(), project.clone()).unwrap();
        let mut stale = project;
        stale.revision = 1;
        assert!(save_project(root.path().display().to_string(), stale).is_err());
        assert_eq!(load(root.path()).unwrap().revision, 2);
    }

    #[test]
    fn failed_command_batch_is_not_partially_saved() {
        let root = tempfile::tempdir().unwrap();
        let project = ProjectDocument::new("Batch test".into());
        create(root.path(), &project).unwrap();
        let batch_id = Uuid::new_v4();
        let asset_id = Uuid::new_v4();
        let envelope = |revision, payload| command::CommandEnvelope {
            command_id: Uuid::new_v4(),
            project_id: project.id,
            source: "manual".into(),
            conversation_id: None,
            batch_id,
            expected_project_revision: revision,
            payload,
        };
        let result = dispatch_batch_persisted(
            root.path(),
            vec![
                envelope(
                    0,
                    command::EditorCommand::AddMedia {
                        asset: Box::new(project::MediaAsset {
                            id: asset_id,
                            name: "Fixture.mp4".into(),
                            kind: "video".into(),
                            path: "Fixture.mp4".into(),
                            duration: project::RationalTime {
                                value: 600,
                                timescale: 600,
                            },
                            width: Some(320),
                            height: Some(180),
                            status: "ready".into(),
                            bookmark: None,
                            color: None,
                            thumbnail_path: None,
                            waveform_path: None,
                            codec: None,
                            has_audio: Some(false),
                            proxy_path: None,
                        }),
                    },
                ),
                envelope(
                    1,
                    command::EditorCommand::AddClip {
                        track_id: Uuid::new_v4(),
                        asset_id,
                        timeline_start: project::RationalTime {
                            value: 0,
                            timescale: 600,
                        },
                    },
                ),
            ],
        );
        assert!(result.is_err());
        let saved = load(root.path()).unwrap();
        assert_eq!(saved.revision, 0);
        assert!(saved.media.is_empty());
    }
}

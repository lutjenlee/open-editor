mod command;
mod media;
mod native;
mod project;

use media::{ExportRequest, MediaInspection};
use project::{
    append_history, canonical_folder, create, existing_folder, load, save_atomic, AnalysisArtifact,
    ProjectDocument,
};
use uuid::Uuid;

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
    save_atomic(&folder, &project)
}

#[tauri::command]
fn record_history(folder: String, entry: serde_json::Value) -> Result<(), project::ProjectError> {
    let folder = existing_folder(&folder)?;
    append_history(&folder, &entry)
}

#[tauri::command]
fn dispatch_editor_command(
    folder: String,
    envelope: command::CommandEnvelope,
) -> Result<command::CommandResult, project::ProjectError> {
    let folder = existing_folder(&folder)?;
    let project = load(&folder)?;
    let result = command::dispatch(project, &envelope)?;
    save_atomic(&folder, &result.project)?;
    append_history(
        &folder,
        &serde_json::json!({ "event": "command", "at": chrono::Utc::now().to_rfc3339(), "envelope": envelope, "newProjectRevision": result.new_project_revision, "affectedEntityIds": result.affected_entity_ids, "forwardPatch": result.forward_patch, "inversePatch": result.inverse_patch }),
    )?;
    Ok(result)
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
    tauri::async_runtime::spawn_blocking(move || media::export(request))
        .await
        .map_err(|error| media::MediaError::Failed(error.to_string()))?
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

#[tauri::command]
async fn create_media_proxy(
    folder: String,
    asset_id: Uuid,
) -> Result<ProjectDocument, media::MediaError> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = existing_folder(&folder)
            .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        let mut project =
            load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
        let source = asset_source(&folder, &project, asset_id)?;
        let proxy_path = media::create_proxy(&source, &folder, asset_id)?;
        let asset = project
            .media
            .iter_mut()
            .find(|asset| asset.id == asset_id)
            .unwrap();
        asset.proxy_path = Some(proxy_path);
        project.revision += 1;
        project.updated_at = chrono::Utc::now().to_rfc3339();
        save_atomic(&folder, &project)
            .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        append_history(
            &folder,
            &serde_json::json!({
                "event": "proxyCreated", "at": project.updated_at, "assetId": asset_id,
                "newProjectRevision": project.revision
            }),
        )
        .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        Ok(project)
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
        let mut project =
            load(&folder).map_err(|error| media::MediaError::Failed(error.to_string()))?;
        let source = asset_source(&folder, &project, asset_id)?;
        let report = media::analyze(&source, &folder, asset_id)?;
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
                data: serde_json::to_value(report.scene_times).unwrap(),
            },
            AnalysisArtifact {
                id: Uuid::new_v4(),
                asset_id,
                kind: "silence".into(),
                status: "ready".into(),
                created_at: now.clone(),
                paths: vec![],
                data: serde_json::to_value(report.silence_ranges).unwrap(),
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
        save_atomic(&folder, &project)
            .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        append_history(
            &folder,
            &serde_json::json!({
                "event": "mediaAnalyzed", "at": project.updated_at, "assetId": asset_id,
                "newProjectRevision": project.revision
            }),
        )
        .map_err(|error| media::MediaError::Failed(error.to_string()))?;
        Ok(project)
    })
    .await
    .map_err(|error| media::MediaError::Failed(error.to_string()))?
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
        .invoke_handler(tauri::generate_handler![
            create_project,
            open_project,
            save_project,
            record_history,
            dispatch_editor_command,
            inspect_media,
            relink_media,
            create_media_proxy,
            analyze_media_asset,
            export_video,
            runtime_capabilities
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Editor");
}

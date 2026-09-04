mod command;
mod media;
mod project;

use media::{ExportRequest, MediaInspection};
use project::{
    append_history, canonical_folder, create, existing_folder, load, save_atomic, ProjectDocument,
};

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
        &serde_json::json!({ "event": "command", "at": chrono::Utc::now().to_rfc3339(), "envelope": envelope, "newProjectRevision": result.new_project_revision, "affectedEntityIds": result.affected_entity_ids }),
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
async fn export_video(request: ExportRequest) -> Result<String, media::MediaError> {
    tauri::async_runtime::spawn_blocking(move || media::export(request))
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
            export_video,
            runtime_capabilities
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Editor");
}

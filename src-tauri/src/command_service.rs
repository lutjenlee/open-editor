use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use uuid::Uuid;

use crate::{
    analyze_persisted, command::CommandEnvelope, create_proxy_persisted, dispatch_batch_persisted,
    dispatch_persisted, jobs::JobManager, project, transcribe_persisted,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub socket_path: String,
    pub capability_token: String,
}

struct Runtime {
    info: ServiceInfo,
    stopping: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = UnixStream::connect(&self.info.socket_path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(&self.info.socket_path);
    }
}

#[derive(Default)]
pub struct CommandService {
    runtime: Mutex<Option<Runtime>>,
    projects: Arc<Mutex<HashMap<Uuid, PathBuf>>>,
    jobs: JobManager,
    job_projects: Arc<Mutex<HashMap<Uuid, Uuid>>>,
    transcription_model: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceRequest {
    token: String,
    action: String,
    project_id: Option<Uuid>,
    envelope: Option<CommandEnvelope>,
    envelopes: Option<Vec<CommandEnvelope>>,
    job_id: Option<Uuid>,
    kind: Option<String>,
    asset_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceResponse {
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

impl CommandService {
    pub fn clear_authorization(&self) -> Result<(), project::ProjectError> {
        self.projects
            .lock()
            .map_err(|_| project::ProjectError::Invalid("command service lock failed".into()))?
            .clear();
        Ok(())
    }

    pub fn authorize(
        &self,
        project_id: Uuid,
        folder: PathBuf,
        transcription_model: Option<PathBuf>,
    ) -> Result<ServiceInfo, project::ProjectError> {
        let mut projects = self
            .projects
            .lock()
            .map_err(|_| project::ProjectError::Invalid("command service lock failed".into()))?;
        projects.clear();
        projects.insert(project_id, folder);
        drop(projects);
        *self.transcription_model.lock().map_err(|_| {
            project::ProjectError::Invalid("command service model lock failed".into())
        })? = transcription_model;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| project::ProjectError::Invalid("command service lock failed".into()))?;
        if let Some(runtime) = runtime.as_ref() {
            return Ok(runtime.info.clone());
        }
        let socket_path = std::env::temp_dir().join(format!("open-editor-{}.sock", Uuid::new_v4()));
        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let info = ServiceInfo {
            socket_path: socket_path.to_string_lossy().into_owned(),
            capability_token: token.clone(),
        };
        let projects = Arc::clone(&self.projects);
        let jobs = self.jobs.clone();
        let job_projects = Arc::clone(&self.job_projects);
        let transcription_model = Arc::clone(&self.transcription_model);
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let worker = thread::Builder::new()
            .name("open-editor-command-service".into())
            .spawn(move || {
                while !worker_stopping.load(std::sync::atomic::Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let projects = Arc::clone(&projects);
                            let token = token.clone();
                            let jobs = jobs.clone();
                            let job_projects = Arc::clone(&job_projects);
                            let transcription_model = Arc::clone(&transcription_model);
                            thread::spawn(move || {
                                handle_connection(
                                    stream,
                                    &token,
                                    &projects,
                                    &jobs,
                                    &job_projects,
                                    &transcription_model,
                                )
                            });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(project::ProjectError::Io)?;
        *runtime = Some(Runtime {
            info: info.clone(),
            stopping,
            worker: Some(worker),
        });
        Ok(info)
    }
}

fn response_error(message: impl Into<String>) -> ServiceResponse {
    ServiceResponse {
        ok: false,
        result: None,
        error: Some(message.into()),
    }
}

fn execute(
    request: ServiceRequest,
    expected_token: &str,
    projects: &Mutex<HashMap<Uuid, PathBuf>>,
    jobs: &JobManager,
    job_projects: &Mutex<HashMap<Uuid, Uuid>>,
    transcription_model: &Mutex<Option<PathBuf>>,
) -> ServiceResponse {
    if !constant_time_eq(request.token.as_bytes(), expected_token.as_bytes()) {
        return response_error("unauthorized command service request");
    }
    let project_id = request
        .envelope
        .as_ref()
        .map(|value| value.project_id)
        .or_else(|| {
            request
                .envelopes
                .as_ref()
                .and_then(|values| values.first())
                .map(|value| value.project_id)
        })
        .or(request.project_id);
    let Some(project_id) = project_id else {
        return response_error("projectId is required");
    };
    let folder = match projects
        .lock()
        .ok()
        .and_then(|items| items.get(&project_id).cloned())
    {
        Some(folder) => folder,
        None => return response_error("project is not authorized for this app session"),
    };
    if request.action == "jobStatus" {
        let owned_job = request.job_id.filter(|id| {
            job_projects
                .lock()
                .ok()
                .and_then(|items| items.get(id).copied())
                == Some(project_id)
        });
        return match owned_job.and_then(|id| jobs.get(id)) {
            Some(record) => ServiceResponse {
                ok: true,
                result: serde_json::to_value(record).ok(),
                error: None,
            },
            None => response_error("job does not exist"),
        };
    }
    if request.action == "cancelJob" {
        let owned_job = request.job_id.filter(|id| {
            job_projects
                .lock()
                .ok()
                .and_then(|items| items.get(id).copied())
                == Some(project_id)
        });
        return match owned_job.and_then(|id| jobs.cancel(id).ok()) {
            Some(record) => ServiceResponse {
                ok: true,
                result: serde_json::to_value(record).ok(),
                error: None,
            },
            None => response_error("job does not exist"),
        };
    }
    if request.action == "startJob" {
        let Some(kind) = request.kind else {
            return response_error("kind is required");
        };
        let Some(asset_id) = request.asset_id else {
            return response_error("assetId is required");
        };
        if !matches!(kind.as_str(), "analysis" | "proxy" | "transcription") {
            return response_error("unsupported media job");
        }
        let model = transcription_model
            .lock()
            .ok()
            .and_then(|value| value.clone());
        if kind == "transcription" && model.as_ref().is_none_or(|path| !path.is_file()) {
            return response_error("the local transcription model is not installed");
        }
        let (record, context) = match jobs.create(&kind, Some(asset_id), None) {
            Ok(value) => value,
            Err(error) => return response_error(error),
        };
        if let Ok(mut items) = job_projects.lock() {
            items.insert(record.id, project_id);
        }
        thread::spawn(move || {
            context.running("Starting local media job", 0.02);
            let result = match kind.as_str() {
                "analysis" => analyze_persisted(folder, asset_id, Some(&context)),
                "proxy" => create_proxy_persisted(folder, asset_id, Some(&context)),
                "transcription" => transcribe_persisted(
                    folder,
                    asset_id,
                    model.expect("model checked before spawning"),
                    Some(&context),
                ),
                _ => unreachable!("job kind validated before spawning"),
            };
            match result {
                Ok(_) if context.is_cancelled() => context.finish_cancelled(),
                Ok(project) => context.complete(serde_json::to_value(project).unwrap_or_default()),
                Err(crate::media::MediaError::Cancelled) => context.finish_cancelled(),
                Err(error) => context.fail(error.to_string()),
            }
        });
        return ServiceResponse {
            ok: true,
            result: serde_json::to_value(record).ok(),
            error: None,
        };
    }
    let result = match request.action.as_str() {
        "snapshot" => project::load(&folder)
            .and_then(|project| serde_json::to_value(project).map_err(project::ProjectError::Json)),
        "command" => match request.envelope {
            Some(envelope) => dispatch_persisted(&folder, envelope).and_then(|result| {
                serde_json::to_value(result).map_err(project::ProjectError::Json)
            }),
            None => return response_error("envelope is required"),
        },
        "batch" => match request.envelopes {
            Some(envelopes) => dispatch_batch_persisted(&folder, envelopes).and_then(|results| {
                serde_json::to_value(results).map_err(project::ProjectError::Json)
            }),
            None => return response_error("envelopes are required"),
        },
        _ => return response_error("unsupported command service action"),
    };
    match result {
        Ok(value) => ServiceResponse {
            ok: true,
            result: Some(value),
            error: None,
        },
        Err(error) => response_error(error.to_string()),
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(*left.get(index).unwrap_or(&0) ^ *right.get(index).unwrap_or(&0));
    }
    difference == 0
}

fn handle_connection(
    mut stream: UnixStream,
    token: &str,
    projects: &Mutex<HashMap<Uuid, PathBuf>>,
    jobs: &JobManager,
    job_projects: &Mutex<HashMap<Uuid, Uuid>>,
    transcription_model: &Mutex<Option<PathBuf>>,
) {
    let mut line = String::new();
    let read = BufReader::new(&stream).read_line(&mut line);
    let response = match read {
        Ok(0) => response_error("empty command service request"),
        Ok(_) => serde_json::from_str::<ServiceRequest>(&line)
            .map(|request| {
                execute(
                    request,
                    token,
                    projects,
                    jobs,
                    job_projects,
                    transcription_model,
                )
            })
            .unwrap_or_else(|error| response_error(format!("invalid request: {error}"))),
        Err(error) => response_error(error.to_string()),
    };
    if let Ok(encoded) = serde_json::to_vec(&response) {
        let _ = stream.write_all(&encoded);
        let _ = stream.write_all(b"\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn socket_requires_token_and_authorized_project() {
        let root = tempfile::tempdir().unwrap();
        let project = project::ProjectDocument::new("Service test".into());
        project::create(root.path(), &project).unwrap();
        let service = CommandService::default();
        let info = service
            .authorize(project.id, root.path().to_path_buf(), None)
            .unwrap();
        assert_eq!(
            fs::metadata(&info.socket_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let request = |token: &str| {
            serde_json::json!({
                "token": token, "action": "snapshot", "projectId": project.id
            })
        };
        let call = |value: serde_json::Value| {
            let mut stream = UnixStream::connect(&info.socket_path).unwrap();
            serde_json::to_writer(&mut stream, &value).unwrap();
            stream.write_all(b"\n").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<serde_json::Value>(&line).unwrap()
        };
        assert_eq!(call(request("wrong"))["ok"], false);
        let accepted = call(request(&info.capability_token));
        assert_eq!(accepted["ok"], true);
        assert_eq!(accepted["result"]["id"], project.id.to_string());
    }
}

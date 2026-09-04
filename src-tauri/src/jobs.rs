use chrono::Utc;
use serde::Serialize;
use std::{
    collections::HashMap,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::Emitter;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub id: Uuid,
    pub kind: String,
    pub asset_id: Option<Uuid>,
    pub status: String,
    pub progress: f64,
    pub message: String,
    pub created_at: String,
    pub updated_at: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

struct JobEntry {
    record: JobRecord,
    cancelled: Arc<AtomicBool>,
    process_id: Option<u32>,
}

#[derive(Clone, Default)]
pub struct JobManager {
    entries: Arc<Mutex<HashMap<Uuid, JobEntry>>>,
}

#[derive(Clone)]
pub struct JobContext {
    id: Uuid,
    manager: JobManager,
    cancelled: Arc<AtomicBool>,
    app: Option<tauri::AppHandle>,
}

impl JobManager {
    pub fn create(
        &self,
        kind: impl Into<String>,
        asset_id: Option<Uuid>,
        app: Option<tauri::AppHandle>,
    ) -> Result<(JobRecord, JobContext), String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        let record = JobRecord {
            id,
            kind: kind.into(),
            asset_id,
            status: "queued".into(),
            progress: 0.0,
            message: "Queued".into(),
            created_at: now.clone(),
            updated_at: now,
            result: None,
            error: None,
        };
        self.entries
            .lock()
            .map_err(|_| "job manager lock failed".to_string())?
            .insert(
                id,
                JobEntry {
                    record: record.clone(),
                    cancelled: Arc::clone(&cancelled),
                    process_id: None,
                },
            );
        Ok((
            record,
            JobContext {
                id,
                manager: self.clone(),
                cancelled,
                app,
            },
        ))
    }

    pub fn get(&self, id: Uuid) -> Option<JobRecord> {
        self.entries
            .lock()
            .ok()?
            .get(&id)
            .map(|entry| entry.record.clone())
    }

    pub fn cancel(&self, id: Uuid) -> Result<JobRecord, String> {
        let (process_id, record) = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| "job manager lock failed".to_string())?;
            let entry = entries
                .get_mut(&id)
                .ok_or_else(|| "job does not exist".to_string())?;
            if matches!(
                entry.record.status.as_str(),
                "completed" | "failed" | "cancelled"
            ) {
                return Ok(entry.record.clone());
            }
            entry.cancelled.store(true, Ordering::Release);
            entry.record.status = "cancelling".into();
            entry.record.message = "Cancelling…".into();
            entry.record.updated_at = Utc::now().to_rfc3339();
            (entry.process_id, entry.record.clone())
        };
        if let Some(process_id) = process_id {
            let _ = Command::new("/bin/kill")
                .args(["-TERM", &process_id.to_string()])
                .status();
        }
        Ok(record)
    }

    fn update(&self, id: Uuid, app: Option<&tauri::AppHandle>, update: impl FnOnce(&mut JobEntry)) {
        let record = self.entries.lock().ok().and_then(|mut entries| {
            let entry = entries.get_mut(&id)?;
            update(entry);
            entry.record.updated_at = Utc::now().to_rfc3339();
            Some(entry.record.clone())
        });
        if let (Some(app), Some(record)) = (app, record) {
            let _ = app.emit("media-job", record);
        }
    }
}

impl JobContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn running(&self, message: impl Into<String>, progress: f64) {
        let message = message.into();
        self.manager.update(self.id, self.app.as_ref(), |entry| {
            entry.record.status = "running".into();
            entry.record.message = message;
            entry.record.progress = progress.clamp(0.0, 1.0);
        });
    }

    pub fn register_process(&self, process_id: Option<u32>) {
        self.manager.update(self.id, self.app.as_ref(), |entry| {
            entry.process_id = process_id;
        });
    }

    pub fn complete(&self, result: serde_json::Value) {
        self.manager.update(self.id, self.app.as_ref(), |entry| {
            entry.process_id = None;
            entry.record.status = "completed".into();
            entry.record.progress = 1.0;
            entry.record.message = "Complete".into();
            entry.record.result = Some(result);
        });
    }

    pub fn fail(&self, error: impl Into<String>) {
        let error = error.into();
        self.manager.update(self.id, self.app.as_ref(), |entry| {
            entry.process_id = None;
            entry.record.status = "failed".into();
            entry.record.message = "Failed".into();
            entry.record.error = Some(error);
        });
    }

    pub fn finish_cancelled(&self) {
        self.manager.update(self.id, self.app.as_ref(), |entry| {
            entry.process_id = None;
            entry.record.status = "cancelled".into();
            entry.record.message = "Cancelled".into();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_terminates_a_registered_process() {
        let manager = JobManager::default();
        let (record, context) = manager.create("test", None, None).unwrap();
        let mut child = Command::new("/bin/sleep").arg("10").spawn().unwrap();
        context.running("Sleeping", 0.1);
        context.register_process(Some(child.id()));
        manager.cancel(record.id).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        assert!(context.is_cancelled());
        context.finish_cancelled();
        assert_eq!(manager.get(record.id).unwrap().status, "cancelled");
    }
}

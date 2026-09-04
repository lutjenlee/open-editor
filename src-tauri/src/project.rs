use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

pub const PROJECT_FILE: &str = "open-editor.project.json";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("The selected folder does not contain {PROJECT_FILE}")]
    NotFound,
    #[error("The selected folder already contains {PROJECT_FILE}; open it instead or choose an empty folder")]
    AlreadyExists,
    #[error("This project uses schema version {found}; this build supports up to {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("Invalid project: {0}")]
    Invalid(String),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Project data could not be decoded: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for ProjectError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RationalTime {
    pub value: i64,
    pub timescale: i32,
}

impl RationalTime {
    pub fn validate(self) -> Result<(), ProjectError> {
        if self.timescale <= 0 {
            return Err(ProjectError::Invalid(
                "time timescale must be positive".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub rotation: f64,
    pub opacity: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioMix {
    pub volume: f64,
    pub fade_in: RationalTime,
    pub fade_out: RationalTime,
    pub ducking: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub name: String,
    pub source_in: RationalTime,
    pub source_out: RationalTime,
    pub timeline_start: RationalTime,
    pub playback_rate: f64,
    pub transform: Transform,
    pub audio: AudioMix,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub locked: bool,
    pub muted: bool,
    pub clips: Vec<Clip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalTime,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub duration: RationalTime,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub status: String,
    #[serde(default)]
    pub bookmark: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub thumbnail_path: Option<String>,
    #[serde(default)]
    pub waveform_path: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub has_audio: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRecord {
    pub id: Uuid,
    pub provider: String,
    pub title: String,
    pub external_thread_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocument {
    pub schema_version: u32,
    pub id: Uuid,
    pub revision: u64,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub media: Vec<MediaAsset>,
    pub sequences: Vec<Sequence>,
    pub active_sequence_id: Uuid,
    pub conversations: Vec<ConversationRecord>,
    pub hosted_context_consent: bool,
}

impl ProjectDocument {
    pub fn new(name: String) -> Self {
        let now = Utc::now().to_rfc3339();
        let sequence_id = Uuid::new_v4();
        let track = |name: &str, kind: &str| Track {
            id: Uuid::new_v4(),
            name: name.into(),
            kind: kind.into(),
            locked: false,
            muted: false,
            clips: vec![],
        };
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            revision: 0,
            name,
            created_at: now.clone(),
            updated_at: now,
            media: vec![],
            sequences: vec![Sequence {
                id: sequence_id,
                name: "Main sequence".into(),
                width: 1080,
                height: 1920,
                frame_rate: RationalTime {
                    value: 30,
                    timescale: 1,
                },
                tracks: vec![
                    track("Video 1", "video"),
                    track("Overlay 1", "overlay"),
                    track("Captions", "caption"),
                    track("Audio 1", "audio"),
                ],
            }],
            active_sequence_id: sequence_id,
            conversations: vec![],
            hosted_context_consent: false,
        }
    }

    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedSchema {
                found: self.schema_version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        if !self
            .sequences
            .iter()
            .any(|sequence| sequence.id == self.active_sequence_id)
        {
            return Err(ProjectError::Invalid(
                "active sequence does not exist".into(),
            ));
        }
        let mut ids = HashSet::new();
        if !ids.insert(self.id) {
            return Err(ProjectError::Invalid("duplicate project identifier".into()));
        }
        let media_ids: HashSet<Uuid> = self.media.iter().map(|asset| asset.id).collect();
        if media_ids.len() != self.media.len() {
            return Err(ProjectError::Invalid("duplicate media identifier".into()));
        }
        for asset in &self.media {
            asset.duration.validate()?;
            if asset.duration.value < 0
                || !matches!(asset.kind.as_str(), "video" | "audio" | "image")
            {
                return Err(ProjectError::Invalid(format!(
                    "media {} is invalid",
                    asset.id
                )));
            }
        }
        for sequence in &self.sequences {
            if !ids.insert(sequence.id) {
                return Err(ProjectError::Invalid(
                    "duplicate sequence identifier".into(),
                ));
            }
            sequence.frame_rate.validate()?;
            if sequence.width == 0 || sequence.height == 0 {
                return Err(ProjectError::Invalid(
                    "sequence dimensions must be positive".into(),
                ));
            }
            for track in &sequence.tracks {
                if !ids.insert(track.id) {
                    return Err(ProjectError::Invalid("duplicate track identifier".into()));
                }
                if !matches!(
                    track.kind.as_str(),
                    "video" | "overlay" | "caption" | "audio"
                ) {
                    return Err(ProjectError::Invalid(format!(
                        "track {} has invalid kind",
                        track.id
                    )));
                }
                for clip in &track.clips {
                    if !ids.insert(clip.id) {
                        return Err(ProjectError::Invalid("duplicate clip identifier".into()));
                    }
                    clip.source_in.validate()?;
                    clip.source_out.validate()?;
                    clip.timeline_start.validate()?;
                    if !media_ids.contains(&clip.asset_id)
                        || clip.source_in.value < 0
                        || time_value(clip.source_out) <= time_value(clip.source_in)
                        || clip.timeline_start.value < 0
                        || clip.playback_rate <= 0.0
                        || clip.transform.scale <= 0.0
                        || !(0.0..=1.0).contains(&clip.transform.opacity)
                        || !(0.0..=4.0).contains(&clip.audio.volume)
                    {
                        return Err(ProjectError::Invalid(format!(
                            "clip {} has invalid playback or opacity",
                            clip.id
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn time_value(time: RationalTime) -> f64 {
    time.value as f64 / time.timescale as f64
}

pub fn initialize_layout(folder: &Path) -> Result<(), ProjectError> {
    let hidden = folder.join(".open-editor");
    for child in [
        "chats",
        "cache",
        "proxies",
        "thumbnails",
        "waveforms",
        "transcripts",
    ] {
        fs::create_dir_all(hidden.join(child))?;
    }
    let ignore = hidden.join(".gitignore");
    if !ignore.exists() {
        fs::write(
            ignore,
            "cache/\nproxies/\nthumbnails/\nwaveforms/\ntranscripts/\n",
        )?;
    }
    let history = hidden.join("history.jsonl");
    if !history.exists() {
        fs::File::create(history)?;
    }
    Ok(())
}

pub fn load(folder: &Path) -> Result<ProjectDocument, ProjectError> {
    let path = folder.join(PROJECT_FILE);
    if !path.exists() {
        return Err(ProjectError::NotFound);
    }
    let mut project: ProjectDocument = serde_json::from_slice(&fs::read(path)?)?;
    project.validate()?;
    for asset in &mut project.media {
        let media_path = Path::new(&asset.path);
        let resolved = if media_path.is_absolute() {
            media_path.to_path_buf()
        } else {
            folder.join(media_path)
        };
        if !resolved.is_file() {
            asset.status = "missing".into();
        }
    }
    Ok(project)
}

pub fn save_atomic(folder: &Path, project: &ProjectDocument) -> Result<(), ProjectError> {
    project.validate()?;
    initialize_layout(folder)?;
    let target = folder.join(PROJECT_FILE);
    let temporary = folder.join(format!(".{PROJECT_FILE}.{}.tmp", Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(project)?;
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, &target)?;
    Ok(())
}

pub fn append_history(folder: &Path, entry: &serde_json::Value) -> Result<(), ProjectError> {
    initialize_layout(folder)?;
    let path = folder.join(".open-editor/history.jsonl");
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, entry)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

pub fn create(folder: &Path, project: &ProjectDocument) -> Result<(), ProjectError> {
    if folder.join(PROJECT_FILE).exists() {
        return Err(ProjectError::AlreadyExists);
    }
    save_atomic(folder, project)
}

pub fn canonical_folder(path: &str) -> Result<PathBuf, ProjectError> {
    let path = PathBuf::from(path);
    fs::create_dir_all(&path)?;
    Ok(path.canonicalize()?)
}

pub fn existing_folder(path: &str) -> Result<PathBuf, ProjectError> {
    Ok(PathBuf::from(path).canonicalize()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_timescale() {
        assert!(RationalTime {
            value: 10,
            timescale: 0
        }
        .validate()
        .is_err());
    }

    #[test]
    fn creates_reconstructable_cache_layout() {
        let root = tempfile::tempdir().unwrap();
        initialize_layout(root.path()).unwrap();
        assert!(root.path().join(".open-editor/proxies").is_dir());
        assert!(root.path().join(".open-editor/.gitignore").is_file());
    }

    #[test]
    fn creates_a_valid_empty_project() {
        let project = ProjectDocument::new("First project".into());
        project.validate().unwrap();
        assert_eq!(project.sequences[0].tracks.len(), 4);
    }

    #[test]
    fn refuses_to_overwrite_an_existing_project_during_creation() {
        let root = tempfile::tempdir().unwrap();
        let project = ProjectDocument::new("First project".into());
        create(root.path(), &project).unwrap();

        let replacement = ProjectDocument::new("Replacement".into());
        assert!(matches!(
            create(root.path(), &replacement),
            Err(ProjectError::AlreadyExists)
        ));
    }
}

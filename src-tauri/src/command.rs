use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::{Clip, MediaAsset, ProjectDocument, ProjectError, RationalTime, Track};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub command_id: Uuid,
    pub project_id: Uuid,
    pub source: String,
    pub conversation_id: Option<Uuid>,
    pub batch_id: Uuid,
    pub expected_project_revision: u64,
    pub payload: EditorCommand,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EditorCommand {
    AddMedia {
        asset: Box<MediaAsset>,
    },
    RemoveMedia {
        asset_id: Uuid,
    },
    AddClip {
        track_id: Uuid,
        asset_id: Uuid,
        timeline_start: RationalTime,
    },
    RemoveClip {
        track_id: Uuid,
        clip_id: Uuid,
    },
    MoveClip {
        track_id: Uuid,
        clip_id: Uuid,
        timeline_start: RationalTime,
    },
    TrimClip {
        track_id: Uuid,
        clip_id: Uuid,
        source_in: RationalTime,
        source_out: RationalTime,
    },
    SplitClip {
        track_id: Uuid,
        clip_id: Uuid,
        at: RationalTime,
    },
    DuplicateClip {
        track_id: Uuid,
        clip_id: Uuid,
        timeline_start: RationalTime,
    },
    ChangeSpeed {
        track_id: Uuid,
        clip_id: Uuid,
        playback_rate: f64,
    },
    SetOpacity {
        track_id: Uuid,
        clip_id: Uuid,
        opacity: f64,
    },
    SetVolume {
        track_id: Uuid,
        clip_id: Uuid,
        volume: f64,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub new_project_revision: u64,
    pub affected_entity_ids: Vec<Uuid>,
    pub project: ProjectDocument,
    pub warnings: Vec<String>,
}

fn time_seconds(time: RationalTime) -> f64 {
    time.value as f64 / time.timescale as f64
}
fn from_seconds(value: f64) -> RationalTime {
    RationalTime {
        value: (value * 600.0).round() as i64,
        timescale: 600,
    }
}

fn active_track(project: &mut ProjectDocument, track_id: Uuid) -> Result<&mut Track, ProjectError> {
    let sequence = project
        .sequences
        .iter_mut()
        .find(|item| item.id == project.active_sequence_id)
        .ok_or_else(|| ProjectError::Invalid("active sequence does not exist".into()))?;
    let track = sequence
        .tracks
        .iter_mut()
        .find(|item| item.id == track_id)
        .ok_or_else(|| ProjectError::Invalid("track does not exist".into()))?;
    if track.locked {
        return Err(ProjectError::Invalid("track is locked".into()));
    }
    Ok(track)
}

fn clip(track: &mut Track, clip_id: Uuid) -> Result<&mut Clip, ProjectError> {
    track
        .clips
        .iter_mut()
        .find(|item| item.id == clip_id)
        .ok_or_else(|| ProjectError::Invalid("clip does not exist".into()))
}

pub fn dispatch(
    mut project: ProjectDocument,
    envelope: &CommandEnvelope,
) -> Result<CommandResult, ProjectError> {
    if envelope.project_id != project.id {
        return Err(ProjectError::Invalid(
            "command targets a different project".into(),
        ));
    }
    if envelope.expected_project_revision != project.revision {
        return Err(ProjectError::Invalid(format!(
            "stale revision: expected {}, current {}",
            envelope.expected_project_revision, project.revision
        )));
    }
    if !matches!(envelope.source.as_str(), "manual" | "codex" | "ollama") {
        return Err(ProjectError::Invalid("invalid command source".into()));
    }
    if envelope.source != "manual"
        && matches!(
            &envelope.payload,
            EditorCommand::AddMedia { .. } | EditorCommand::RemoveMedia { .. }
        )
    {
        return Err(ProjectError::Invalid(
            "providers cannot change the project's approved media scope".into(),
        ));
    }
    let mut affected = Vec::new();
    match &envelope.payload {
        EditorCommand::AddMedia { asset } => {
            if project.media.iter().any(|item| item.id == asset.id) {
                return Err(ProjectError::Invalid(
                    "media identifier already exists".into(),
                ));
            }
            project.media.push((**asset).clone());
            affected.push(asset.id);
        }
        EditorCommand::RemoveMedia { asset_id } => {
            if project.sequences.iter().any(|sequence| {
                sequence
                    .tracks
                    .iter()
                    .any(|track| track.clips.iter().any(|item| item.asset_id == *asset_id))
            }) {
                return Err(ProjectError::Invalid("media is in use".into()));
            }
            project.media.retain(|item| item.id != *asset_id);
            affected.push(*asset_id);
        }
        EditorCommand::AddClip {
            track_id,
            asset_id,
            timeline_start,
        } => {
            let asset = project
                .media
                .iter()
                .find(|item| item.id == *asset_id)
                .cloned()
                .ok_or_else(|| ProjectError::Invalid("media does not exist".into()))?;
            let id = Uuid::new_v4();
            let duration = if time_seconds(asset.duration) > 0.0 {
                asset.duration
            } else {
                from_seconds(5.0)
            };
            let new_clip = Clip {
                id,
                asset_id: *asset_id,
                name: asset.name.trim_end_matches(|c| c != '.').to_string(),
                source_in: from_seconds(0.0),
                source_out: duration,
                timeline_start: *timeline_start,
                playback_rate: 1.0,
                transform: crate::project::Transform {
                    x: 0.0,
                    y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                    opacity: 1.0,
                },
                audio: crate::project::AudioMix {
                    volume: 1.0,
                    fade_in: from_seconds(0.0),
                    fade_out: from_seconds(0.0),
                    ducking: false,
                },
                color: asset.color.unwrap_or_else(|| "#6f7fc4".into()),
            };
            active_track(&mut project, *track_id)?.clips.push(new_clip);
            affected.push(id);
        }
        EditorCommand::RemoveClip { track_id, clip_id } => {
            active_track(&mut project, *track_id)?
                .clips
                .retain(|item| item.id != *clip_id);
            affected.push(*clip_id);
        }
        EditorCommand::MoveClip {
            track_id,
            clip_id,
            timeline_start,
        } => {
            if timeline_start.value < 0 {
                return Err(ProjectError::Invalid(
                    "clip cannot start before timeline".into(),
                ));
            }
            clip(active_track(&mut project, *track_id)?, *clip_id)?.timeline_start =
                *timeline_start;
            affected.push(*clip_id);
        }
        EditorCommand::TrimClip {
            track_id,
            clip_id,
            source_in,
            source_out,
        } => {
            if time_seconds(*source_out) <= time_seconds(*source_in) {
                return Err(ProjectError::Invalid(
                    "trim end must follow trim start".into(),
                ));
            }
            let item = clip(active_track(&mut project, *track_id)?, *clip_id)?;
            item.source_in = *source_in;
            item.source_out = *source_out;
            affected.push(*clip_id);
        }
        EditorCommand::SplitClip {
            track_id,
            clip_id,
            at,
        } => {
            let track = active_track(&mut project, *track_id)?;
            let index = track
                .clips
                .iter()
                .position(|item| item.id == *clip_id)
                .ok_or_else(|| ProjectError::Invalid("clip does not exist".into()))?;
            let original = track.clips[index].clone();
            let offset = time_seconds(*at) - time_seconds(original.timeline_start);
            let duration = (time_seconds(original.source_out) - time_seconds(original.source_in))
                / original.playback_rate;
            if offset <= 1.0 / 30.0 || offset >= duration - 1.0 / 30.0 {
                return Err(ProjectError::Invalid(
                    "split point must be inside clip".into(),
                ));
            }
            let source_split =
                from_seconds(time_seconds(original.source_in) + offset * original.playback_rate);
            track.clips[index].source_out = source_split;
            let mut right = original;
            right.id = Uuid::new_v4();
            right.name = format!("{} B", right.name);
            right.source_in = source_split;
            right.timeline_start = *at;
            let right_id = right.id;
            track.clips.insert(index + 1, right);
            affected.extend([*clip_id, right_id]);
        }
        EditorCommand::DuplicateClip {
            track_id,
            clip_id,
            timeline_start,
        } => {
            let track = active_track(&mut project, *track_id)?;
            let mut copy = track
                .clips
                .iter()
                .find(|item| item.id == *clip_id)
                .cloned()
                .ok_or_else(|| ProjectError::Invalid("clip does not exist".into()))?;
            copy.id = Uuid::new_v4();
            copy.name = format!("{} copy", copy.name);
            copy.timeline_start = *timeline_start;
            affected.push(copy.id);
            track.clips.push(copy);
        }
        EditorCommand::ChangeSpeed {
            track_id,
            clip_id,
            playback_rate,
        } => {
            if !(0.1..=8.0).contains(playback_rate) {
                return Err(ProjectError::Invalid(
                    "speed must be between 0.1 and 8".into(),
                ));
            }
            clip(active_track(&mut project, *track_id)?, *clip_id)?.playback_rate = *playback_rate;
            affected.push(*clip_id);
        }
        EditorCommand::SetOpacity {
            track_id,
            clip_id,
            opacity,
        } => {
            if !(0.0..=1.0).contains(opacity) {
                return Err(ProjectError::Invalid(
                    "opacity must be between 0 and 1".into(),
                ));
            }
            clip(active_track(&mut project, *track_id)?, *clip_id)?
                .transform
                .opacity = *opacity;
            affected.push(*clip_id);
        }
        EditorCommand::SetVolume {
            track_id,
            clip_id,
            volume,
        } => {
            if !(0.0..=4.0).contains(volume) {
                return Err(ProjectError::Invalid(
                    "volume must be between 0 and 4".into(),
                ));
            }
            clip(active_track(&mut project, *track_id)?, *clip_id)?
                .audio
                .volume = *volume;
            affected.push(*clip_id);
        }
    }
    project.revision += 1;
    project.updated_at = Utc::now().to_rfc3339();
    project.validate()?;
    Ok(CommandResult {
        new_project_revision: project.revision,
        affected_entity_ids: affected,
        project,
        warnings: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_stale_provider_commands() {
        let project = ProjectDocument::new("Test".into());
        let envelope = CommandEnvelope {
            command_id: Uuid::new_v4(),
            project_id: project.id,
            source: "codex".into(),
            conversation_id: None,
            batch_id: Uuid::new_v4(),
            expected_project_revision: 99,
            payload: EditorCommand::RemoveMedia {
                asset_id: Uuid::new_v4(),
            },
        };
        assert!(dispatch(project, &envelope).is_err());
    }

    #[test]
    fn manual_import_uses_the_dispatcher() {
        {
            let source = "manual";
            let project = ProjectDocument::new("Test".into());
            let asset_id = Uuid::new_v4();
            let asset = MediaAsset {
                id: asset_id,
                name: "clip.mp4".into(),
                kind: "video".into(),
                path: "/tmp/clip.mp4".into(),
                duration: from_seconds(1.0),
                width: Some(320),
                height: Some(180),
                status: "ready".into(),
                bookmark: None,
                color: None,
                thumbnail_path: None,
                waveform_path: None,
                codec: Some("h264".into()),
                has_audio: Some(false),
            };
            let envelope = CommandEnvelope {
                command_id: Uuid::new_v4(),
                project_id: project.id,
                source: source.into(),
                conversation_id: None,
                batch_id: Uuid::new_v4(),
                expected_project_revision: 0,
                payload: EditorCommand::AddMedia {
                    asset: Box::new(asset),
                },
            };
            let result = dispatch(project, &envelope).unwrap();
            assert_eq!(result.project.media[0].id, asset_id);
        }
    }

    #[test]
    fn providers_cannot_expand_approved_media_scope() {
        let project = ProjectDocument::new("Test".into());
        let asset = MediaAsset {
            id: Uuid::new_v4(),
            name: "outside.mp4".into(),
            kind: "video".into(),
            path: "/tmp/outside.mp4".into(),
            duration: from_seconds(1.0),
            width: None,
            height: None,
            status: "ready".into(),
            bookmark: None,
            color: None,
            thumbnail_path: None,
            waveform_path: None,
            codec: None,
            has_audio: None,
        };
        let envelope = CommandEnvelope {
            command_id: Uuid::new_v4(),
            project_id: project.id,
            source: "codex".into(),
            conversation_id: None,
            batch_id: Uuid::new_v4(),
            expected_project_revision: 0,
            payload: EditorCommand::AddMedia {
                asset: Box::new(asset),
            },
        };
        assert!(dispatch(project, &envelope).is_err());
    }

    #[test]
    fn accepts_the_frontend_camel_case_envelope() {
        let project = ProjectDocument::new("Test".into());
        let value = serde_json::json!({
            "commandId": Uuid::new_v4(), "projectId": project.id, "source": "manual",
            "batchId": Uuid::new_v4(), "expectedProjectRevision": 0,
            "payload": { "type": "moveClip", "trackId": project.sequences[0].tracks[0].id, "clipId": Uuid::new_v4(), "timelineStart": { "value": 0, "timescale": 600 } }
        });
        let decoded: CommandEnvelope = serde_json::from_value(value).unwrap();
        assert!(matches!(decoded.payload, EditorCommand::MoveClip { .. }));
    }
}

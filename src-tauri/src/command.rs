use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::project::{
    CaptionSegment, CaptionStyle, Clip, MediaAsset, ProjectDocument, ProjectError, RationalTime,
    Sequence, Track, Transition,
};

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
    DuplicateSequence {
        sequence_id: Uuid,
        name: String,
    },
    SetActiveSequence {
        sequence_id: Uuid,
    },
    RenameSequence {
        sequence_id: Uuid,
        name: String,
    },
    RemoveSequence {
        sequence_id: Uuid,
    },
    SetTrackLocked {
        track_id: Uuid,
        locked: bool,
    },
    SetTrackMuted {
        track_id: Uuid,
        muted: bool,
    },
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
        timeline_start: Option<RationalTime>,
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
    FadeAudio {
        track_id: Uuid,
        clip_id: Uuid,
        fade_in: RationalTime,
        fade_out: RationalTime,
    },
    DuckAudio {
        track_id: Uuid,
        clip_id: Uuid,
        enabled: bool,
    },
    ReplaceClip {
        track_id: Uuid,
        clip_id: Uuid,
        asset_id: Uuid,
    },
    CropClip {
        track_id: Uuid,
        clip_id: Uuid,
        transform: crate::project::Transform,
    },
    AddCaption {
        track_id: Uuid,
        start: RationalTime,
        end: RationalTime,
        text: String,
    },
    EditCaption {
        caption_id: Uuid,
        text: String,
    },
    StyleCaption {
        caption_id: Uuid,
        style: CaptionStyle,
    },
    RemoveCaption {
        caption_id: Uuid,
    },
    AddTransition {
        from_clip_id: Uuid,
        to_clip_id: Uuid,
        kind: String,
        duration: RationalTime,
    },
    RemoveTransition {
        transition_id: Uuid,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub new_project_revision: u64,
    pub affected_entity_ids: Vec<Uuid>,
    pub project: ProjectDocument,
    pub forward_patch: ProjectPatch,
    pub inverse_patch: ProjectPatch,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub before: ProjectDocument,
    pub after: ProjectDocument,
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

fn active_sequence(project: &mut ProjectDocument) -> Result<&mut Sequence, ProjectError> {
    project
        .sequences
        .iter_mut()
        .find(|item| item.id == project.active_sequence_id)
        .ok_or_else(|| ProjectError::Invalid("active sequence does not exist".into()))
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
    let before = project.clone();
    let mut affected = Vec::new();
    match &envelope.payload {
        EditorCommand::DuplicateSequence { sequence_id, name } => {
            if name.trim().is_empty() {
                return Err(ProjectError::Invalid(
                    "sequence name cannot be empty".into(),
                ));
            }
            let mut sequence = project
                .sequences
                .iter()
                .find(|item| item.id == *sequence_id)
                .cloned()
                .ok_or_else(|| ProjectError::Invalid("sequence does not exist".into()))?;
            let new_sequence_id = Uuid::new_v4();
            let mut track_ids = HashMap::new();
            let mut clip_ids = HashMap::new();
            sequence.id = new_sequence_id;
            sequence.name = name.trim().into();
            for track in &mut sequence.tracks {
                let old = track.id;
                track.id = Uuid::new_v4();
                track_ids.insert(old, track.id);
                for clip in &mut track.clips {
                    let old = clip.id;
                    clip.id = Uuid::new_v4();
                    clip_ids.insert(old, clip.id);
                }
            }
            for caption in &mut sequence.captions {
                caption.id = Uuid::new_v4();
                caption.track_id = *track_ids
                    .get(&caption.track_id)
                    .ok_or_else(|| ProjectError::Invalid("caption track is invalid".into()))?;
            }
            for transition in &mut sequence.transitions {
                transition.id = Uuid::new_v4();
                transition.from_clip_id =
                    *clip_ids.get(&transition.from_clip_id).ok_or_else(|| {
                        ProjectError::Invalid("transition source clip is invalid".into())
                    })?;
                transition.to_clip_id = *clip_ids.get(&transition.to_clip_id).ok_or_else(|| {
                    ProjectError::Invalid("transition destination clip is invalid".into())
                })?;
            }
            project.sequences.push(sequence);
            project.active_sequence_id = new_sequence_id;
            affected.push(new_sequence_id);
        }
        EditorCommand::SetActiveSequence { sequence_id } => {
            if !project.sequences.iter().any(|item| item.id == *sequence_id) {
                return Err(ProjectError::Invalid("sequence does not exist".into()));
            }
            project.active_sequence_id = *sequence_id;
            affected.push(*sequence_id);
        }
        EditorCommand::RenameSequence { sequence_id, name } => {
            if name.trim().is_empty() {
                return Err(ProjectError::Invalid(
                    "sequence name cannot be empty".into(),
                ));
            }
            let sequence = project
                .sequences
                .iter_mut()
                .find(|item| item.id == *sequence_id)
                .ok_or_else(|| ProjectError::Invalid("sequence does not exist".into()))?;
            sequence.name = name.trim().into();
            affected.push(*sequence_id);
        }
        EditorCommand::RemoveSequence { sequence_id } => {
            if project.sequences.len() <= 1 {
                return Err(ProjectError::Invalid(
                    "a project must keep one sequence".into(),
                ));
            }
            if !project.sequences.iter().any(|item| item.id == *sequence_id) {
                return Err(ProjectError::Invalid("sequence does not exist".into()));
            }
            project.sequences.retain(|item| item.id != *sequence_id);
            if project.active_sequence_id == *sequence_id {
                project.active_sequence_id = project.sequences[0].id;
            }
            affected.push(*sequence_id);
        }
        EditorCommand::SetTrackLocked { track_id, locked } => {
            let track = active_sequence(&mut project)?
                .tracks
                .iter_mut()
                .find(|item| item.id == *track_id)
                .ok_or_else(|| ProjectError::Invalid("track does not exist".into()))?;
            track.locked = *locked;
            affected.push(*track_id);
        }
        EditorCommand::SetTrackMuted { track_id, muted } => {
            let track = active_sequence(&mut project)?
                .tracks
                .iter_mut()
                .find(|item| item.id == *track_id)
                .ok_or_else(|| ProjectError::Invalid("track does not exist".into()))?;
            track.muted = *muted;
            affected.push(*track_id);
        }
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
            active_sequence(&mut project)?
                .transitions
                .retain(|transition| {
                    transition.from_clip_id != *clip_id && transition.to_clip_id != *clip_id
                });
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
            timeline_start,
        } => {
            if time_seconds(*source_out) <= time_seconds(*source_in) {
                return Err(ProjectError::Invalid(
                    "trim end must follow trim start".into(),
                ));
            }
            let item = clip(active_track(&mut project, *track_id)?, *clip_id)?;
            item.source_in = *source_in;
            item.source_out = *source_out;
            if let Some(timeline_start) = timeline_start {
                if timeline_start.value < 0 {
                    return Err(ProjectError::Invalid(
                        "clip cannot start before timeline".into(),
                    ));
                }
                item.timeline_start = *timeline_start;
            }
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
        EditorCommand::FadeAudio {
            track_id,
            clip_id,
            fade_in,
            fade_out,
        } => {
            if fade_in.value < 0 || fade_out.value < 0 {
                return Err(ProjectError::Invalid(
                    "audio fades cannot be negative".into(),
                ));
            }
            let item = clip(active_track(&mut project, *track_id)?, *clip_id)?;
            let duration =
                (time_seconds(item.source_out) - time_seconds(item.source_in)) / item.playback_rate;
            if time_seconds(*fade_in) + time_seconds(*fade_out) > duration {
                return Err(ProjectError::Invalid(
                    "audio fades cannot exceed the clip duration".into(),
                ));
            }
            item.audio.fade_in = *fade_in;
            item.audio.fade_out = *fade_out;
            affected.push(*clip_id);
        }
        EditorCommand::DuckAudio {
            track_id,
            clip_id,
            enabled,
        } => {
            clip(active_track(&mut project, *track_id)?, *clip_id)?
                .audio
                .ducking = *enabled;
            affected.push(*clip_id);
        }
        EditorCommand::ReplaceClip {
            track_id,
            clip_id,
            asset_id,
        } => {
            let replacement = project
                .media
                .iter()
                .find(|asset| asset.id == *asset_id)
                .cloned()
                .ok_or_else(|| ProjectError::Invalid("replacement media does not exist".into()))?;
            let current_asset_id = active_track(&mut project, *track_id)?
                .clips
                .iter()
                .find(|item| item.id == *clip_id)
                .map(|item| item.asset_id)
                .ok_or_else(|| ProjectError::Invalid("clip does not exist".into()))?;
            let current_kind = project
                .media
                .iter()
                .find(|asset| asset.id == current_asset_id)
                .map(|asset| asset.kind.as_str());
            if current_kind != Some(replacement.kind.as_str()) {
                return Err(ProjectError::Invalid(
                    "replacement media must have the same type".into(),
                ));
            }
            let item = clip(active_track(&mut project, *track_id)?, *clip_id)?;
            item.asset_id = replacement.id;
            item.name = replacement
                .name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(&replacement.name)
                .into();
            item.source_in = from_seconds(0.0);
            item.source_out = if replacement.duration.value > 0 {
                replacement.duration
            } else {
                from_seconds(5.0)
            };
            affected.extend([*clip_id, *asset_id]);
        }
        EditorCommand::CropClip {
            track_id,
            clip_id,
            transform,
        } => {
            if transform.scale <= 0.0 || !(0.0..=1.0).contains(&transform.opacity) {
                return Err(ProjectError::Invalid("clip transform is invalid".into()));
            }
            clip(active_track(&mut project, *track_id)?, *clip_id)?.transform = transform.clone();
            affected.push(*clip_id);
        }
        EditorCommand::AddCaption {
            track_id,
            start,
            end,
            text,
        } => {
            if text.trim().is_empty() || time_seconds(*end) <= time_seconds(*start) {
                return Err(ProjectError::Invalid(
                    "caption text or timing is invalid".into(),
                ));
            }
            let track = active_track(&mut project, *track_id)?;
            if track.kind != "caption" {
                return Err(ProjectError::Invalid(
                    "captions require a caption track".into(),
                ));
            }
            let id = Uuid::new_v4();
            active_sequence(&mut project)?
                .captions
                .push(CaptionSegment {
                    id,
                    track_id: *track_id,
                    start: *start,
                    end: *end,
                    text: text.trim().into(),
                    style: CaptionStyle {
                        font_size: 48.0,
                        color: "#ffffff".into(),
                        background: "#000000".into(),
                        position: "bottom".into(),
                    },
                });
            affected.push(id);
        }
        EditorCommand::EditCaption { caption_id, text } => {
            if text.trim().is_empty() {
                return Err(ProjectError::Invalid("caption text cannot be empty".into()));
            }
            let caption = active_sequence(&mut project)?
                .captions
                .iter_mut()
                .find(|caption| caption.id == *caption_id)
                .ok_or_else(|| ProjectError::Invalid("caption does not exist".into()))?;
            caption.text = text.trim().into();
            affected.push(*caption_id);
        }
        EditorCommand::StyleCaption { caption_id, style } => {
            if style.font_size <= 0.0
                || !matches!(style.position.as_str(), "top" | "center" | "bottom")
            {
                return Err(ProjectError::Invalid("caption style is invalid".into()));
            }
            let caption = active_sequence(&mut project)?
                .captions
                .iter_mut()
                .find(|caption| caption.id == *caption_id)
                .ok_or_else(|| ProjectError::Invalid("caption does not exist".into()))?;
            caption.style = style.clone();
            affected.push(*caption_id);
        }
        EditorCommand::RemoveCaption { caption_id } => {
            let sequence = active_sequence(&mut project)?;
            if !sequence
                .captions
                .iter()
                .any(|caption| caption.id == *caption_id)
            {
                return Err(ProjectError::Invalid("caption does not exist".into()));
            }
            sequence
                .captions
                .retain(|caption| caption.id != *caption_id);
            affected.push(*caption_id);
        }
        EditorCommand::AddTransition {
            from_clip_id,
            to_clip_id,
            kind,
            duration,
        } => {
            if !matches!(kind.as_str(), "cut" | "fade" | "crossDissolve") || duration.value <= 0 {
                return Err(ProjectError::Invalid("transition is invalid".into()));
            }
            let sequence = active_sequence(&mut project)?;
            let has_clip = |id: Uuid| {
                sequence
                    .tracks
                    .iter()
                    .any(|track| track.clips.iter().any(|clip| clip.id == id))
            };
            if from_clip_id == to_clip_id || !has_clip(*from_clip_id) || !has_clip(*to_clip_id) {
                return Err(ProjectError::Invalid("transition clips are invalid".into()));
            }
            let id = Uuid::new_v4();
            sequence.transitions.push(Transition {
                id,
                from_clip_id: *from_clip_id,
                to_clip_id: *to_clip_id,
                kind: kind.clone(),
                duration: *duration,
            });
            affected.push(id);
        }
        EditorCommand::RemoveTransition { transition_id } => {
            let sequence = active_sequence(&mut project)?;
            if !sequence
                .transitions
                .iter()
                .any(|transition| transition.id == *transition_id)
            {
                return Err(ProjectError::Invalid("transition does not exist".into()));
            }
            sequence
                .transitions
                .retain(|transition| transition.id != *transition_id);
            affected.push(*transition_id);
        }
    }
    project.revision += 1;
    project.updated_at = Utc::now().to_rfc3339();
    project.validate()?;
    let after = project.clone();
    Ok(CommandResult {
        new_project_revision: project.revision,
        affected_entity_ids: affected,
        project,
        forward_patch: ProjectPatch {
            before: before.clone(),
            after: after.clone(),
        },
        inverse_patch: ProjectPatch {
            before: after,
            after: before,
        },
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
                proxy_path: None,
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
            proxy_path: None,
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

    #[test]
    fn extended_commands_preserve_exact_inverse_snapshots() {
        let mut project = ProjectDocument::new("Test".into());
        let asset_id = Uuid::new_v4();
        project.media.push(MediaAsset {
            id: asset_id,
            name: "clip.mp4".into(),
            kind: "video".into(),
            path: "/tmp/clip.mp4".into(),
            duration: from_seconds(3.0),
            width: Some(320),
            height: Some(180),
            status: "ready".into(),
            bookmark: None,
            color: None,
            thumbnail_path: None,
            waveform_path: None,
            codec: Some("h264".into()),
            has_audio: Some(true),
            proxy_path: None,
        });
        let video_track = project.sequences[0].tracks[0].id;
        let before = project.clone();
        let added = dispatch(
            project,
            &CommandEnvelope {
                command_id: Uuid::new_v4(),
                project_id: before.id,
                source: "manual".into(),
                conversation_id: None,
                batch_id: Uuid::new_v4(),
                expected_project_revision: 0,
                payload: EditorCommand::AddClip {
                    track_id: video_track,
                    asset_id,
                    timeline_start: from_seconds(0.0),
                },
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&added.inverse_patch.after).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        let clip_id = added.project.sequences[0].tracks[0].clips[0].id;
        let revision = added.project.revision;
        let project_id = added.project.id;
        let cropped = dispatch(
            added.project,
            &CommandEnvelope {
                command_id: Uuid::new_v4(),
                project_id,
                source: "codex".into(),
                conversation_id: Some(Uuid::new_v4()),
                batch_id: Uuid::new_v4(),
                expected_project_revision: revision,
                payload: EditorCommand::CropClip {
                    track_id: video_track,
                    clip_id,
                    transform: crate::project::Transform {
                        x: 10.0,
                        y: -5.0,
                        scale: 1.2,
                        rotation: 0.0,
                        opacity: 0.8,
                    },
                },
            },
        )
        .unwrap();
        assert_eq!(
            cropped.project.sequences[0].tracks[0].clips[0]
                .transform
                .scale,
            1.2
        );
        assert_eq!(cropped.inverse_patch.after.revision, revision);
    }
}

import { seconds, toSeconds } from "./time";
import type { Clip, CommandEnvelope, CommandResult, EditorCommand, ProjectDocument, Track } from "../types/project";

export class CommandError extends Error {}

const copy = <T>(value: T): T => structuredClone(value);

function activeSequence(project: ProjectDocument) {
  const sequence = project.sequences.find((item) => item.id === project.activeSequenceId);
  if (!sequence) throw new CommandError("The active sequence does not exist.");
  return sequence;
}

function findTrack(project: ProjectDocument, trackId: string): Track {
  const track = activeSequence(project).tracks.find((item) => item.id === trackId);
  if (!track) throw new CommandError("The requested track does not exist.");
  if (track.locked) throw new CommandError("The requested track is locked.");
  return track;
}

function findClip(track: Track, clipId: string): Clip {
  const clip = track.clips.find((item) => item.id === clipId);
  if (!clip) throw new CommandError("The requested clip does not exist.");
  return clip;
}

function defaultClip(assetId: string, name: string, duration: number, start: number): Clip {
  return {
    id: crypto.randomUUID(), assetId, name: name.replace(/\.[^.]+$/, ""),
    sourceIn: seconds(0), sourceOut: seconds(Math.max(duration, 1 / 30)), timelineStart: seconds(start),
    playbackRate: 1, transform: { x: 0, y: 0, scale: 1, rotation: 0, opacity: 1 },
    audio: { volume: 1, fadeIn: seconds(0), fadeOut: seconds(0), ducking: false }, color: "#6f7fc4",
  };
}

export function applyEditorCommand(current: ProjectDocument, envelope: CommandEnvelope): CommandResult {
  if (envelope.projectId !== current.id) throw new CommandError("Command targets a different project.");
  if (envelope.expectedProjectRevision !== current.revision) {
    throw new CommandError(`Stale project revision: expected ${envelope.expectedProjectRevision}, current ${current.revision}.`);
  }
  const before = copy(current);
  const project = copy(current);
  const affected: string[] = [];
  const command: EditorCommand = envelope.payload;

  if (command.type === "addMedia") {
    if (project.media.some((item) => item.id === command.asset.id)) throw new CommandError("Media identifier already exists.");
    project.media.push(command.asset); affected.push(command.asset.id);
  } else if (command.type === "removeMedia") {
    const used = project.sequences.some((sequence) => sequence.tracks.some((track) => track.clips.some((clip) => clip.assetId === command.assetId)));
    if (used) throw new CommandError("Remove clips using this media before removing it from the library.");
    project.media = project.media.filter((item) => item.id !== command.assetId); affected.push(command.assetId);
  } else {
    const track = findTrack(project, command.trackId);
    if (command.type === "addClip") {
      const asset = project.media.find((item) => item.id === command.assetId);
      if (!asset) throw new CommandError("The selected media is missing from this project.");
      if (track.kind === "audio" && asset.kind !== "audio") throw new CommandError("Only audio can be added to this track.");
      if (track.kind !== "audio" && asset.kind === "audio") throw new CommandError("Audio must be added to an audio track.");
      const clip = defaultClip(asset.id, asset.name, toSeconds(asset.duration), toSeconds(command.timelineStart));
      clip.color = asset.color ?? clip.color; track.clips.push(clip); affected.push(clip.id);
    } else {
      const clip = findClip(track, command.clipId);
      if (command.type === "removeClip") {
        track.clips = track.clips.filter((item) => item.id !== clip.id); affected.push(clip.id);
      } else if (command.type === "moveClip") {
        if (command.timelineStart.value < 0) throw new CommandError("A clip cannot start before the timeline.");
        clip.timelineStart = command.timelineStart; affected.push(clip.id);
      } else if (command.type === "trimClip") {
        if (toSeconds(command.sourceOut) <= toSeconds(command.sourceIn)) throw new CommandError("Trim end must be after trim start.");
        clip.sourceIn = command.sourceIn; clip.sourceOut = command.sourceOut; affected.push(clip.id);
      } else if (command.type === "splitClip") {
        const timelineOffset = toSeconds(command.at) - toSeconds(clip.timelineStart);
        const duration = (toSeconds(clip.sourceOut) - toSeconds(clip.sourceIn)) / clip.playbackRate;
        if (timelineOffset <= 1 / 30 || timelineOffset >= duration - 1 / 30) throw new CommandError("Move the playhead inside the selected clip before splitting.");
        const sourceSplit = toSeconds(clip.sourceIn) + timelineOffset * clip.playbackRate;
        const right = copy(clip); right.id = crypto.randomUUID(); right.name = `${clip.name} B`;
        right.sourceIn = seconds(sourceSplit); right.timelineStart = command.at; clip.sourceOut = seconds(sourceSplit);
        const index = track.clips.findIndex((item) => item.id === clip.id); track.clips.splice(index + 1, 0, right);
        affected.push(clip.id, right.id);
      } else if (command.type === "duplicateClip") {
        const duplicate = copy(clip); duplicate.id = crypto.randomUUID(); duplicate.name = `${clip.name} copy`; duplicate.timelineStart = command.timelineStart;
        track.clips.push(duplicate); affected.push(duplicate.id);
      } else if (command.type === "changeSpeed") {
        if (command.playbackRate < 0.1 || command.playbackRate > 8) throw new CommandError("Playback speed must be between 0.1× and 8×.");
        clip.playbackRate = command.playbackRate; affected.push(clip.id);
      } else if (command.type === "cropClip") {
        if (command.transform.scale <= 0 || command.transform.opacity < 0 || command.transform.opacity > 1) throw new CommandError("Invalid clip transform.");
        clip.transform = command.transform; affected.push(clip.id);
      } else if (command.type === "setOpacity") {
        if (command.opacity < 0 || command.opacity > 1) throw new CommandError("Opacity must be between 0 and 1.");
        clip.transform.opacity = command.opacity; affected.push(clip.id);
      } else if (command.type === "setVolume") {
        if (command.volume < 0 || command.volume > 4) throw new CommandError("Volume must be between 0 and 4.");
        clip.audio.volume = command.volume; affected.push(clip.id);
      }
    }
  }

  project.revision += 1;
  project.updatedAt = new Date().toISOString();
  const after = copy(project);
  return {
    newProjectRevision: project.revision, affectedEntityIds: affected,
    forwardPatch: { before, after }, inversePatch: { before: after, after: before }, warnings: [],
  };
}

export function createEnvelope(project: ProjectDocument, payload: EditorCommand, source: CommandEnvelope["source"] = "manual", batchId = crypto.randomUUID()): CommandEnvelope {
  return { commandId: crypto.randomUUID(), projectId: project.id, source, batchId, expectedProjectRevision: project.revision, payload };
}

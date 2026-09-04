import { seconds } from "./time";
import type { ProjectDocument, Track, TrackKind } from "../types/project";

function id(): string {
  return crypto.randomUUID();
}

export function createEmptyProject(): ProjectDocument {
  const now = new Date().toISOString();
  const sequenceId = id();
  const track = (name: string, kind: TrackKind): Track => ({ id: id(), name, kind, locked: false, muted: false, clips: [] });
  return {
    schemaVersion: 1,
    id: id(),
    revision: 0,
    name: "No project open",
    createdAt: now,
    updatedAt: now,
    media: [],
    sequences: [{
      id: sequenceId,
      name: "Main sequence",
      width: 1080,
      height: 1920,
      frameRate: { value: 30, timescale: 1 },
      tracks: [track("Video 1", "video"), track("Overlay 1", "overlay"), track("Captions", "caption"), track("Audio 1", "audio")],
      captions: [],
      transitions: [],
    }],
    activeSequenceId: sequenceId,
    conversations: [],
    hostedContextConsent: false,
    analysisArtifacts: [],
  };
}

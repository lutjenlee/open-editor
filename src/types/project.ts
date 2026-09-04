export type Id = string;

export interface RationalTime {
  value: number;
  timescale: number;
}

export type MediaKind = "video" | "audio" | "image";
export type TrackKind = "video" | "overlay" | "caption" | "audio";
export type AgentProvider = "codex" | "ollama";

export interface MediaAsset {
  id: Id;
  name: string;
  kind: MediaKind;
  path: string;
  duration: RationalTime;
  width?: number;
  height?: number;
  status: "ready" | "analyzing" | "missing";
  color?: string;
  bookmark?: string;
  thumbnailPath?: string;
  waveformPath?: string;
  codec?: string;
  hasAudio?: boolean;
  proxyPath?: string;
}

export interface AnalysisArtifact {
  id: Id;
  assetId: Id;
  kind: "scenes" | "silence" | "keyframes" | "transcript";
  status: "ready" | "failed";
  createdAt: string;
  paths: string[];
  data: unknown;
}

export interface Transform {
  x: number;
  y: number;
  scale: number;
  rotation: number;
  opacity: number;
}

export interface AudioMix {
  volume: number;
  fadeIn: RationalTime;
  fadeOut: RationalTime;
  ducking: boolean;
}

export interface Clip {
  id: Id;
  assetId: Id;
  name: string;
  sourceIn: RationalTime;
  sourceOut: RationalTime;
  timelineStart: RationalTime;
  playbackRate: number;
  transform: Transform;
  audio: AudioMix;
  color: string;
}

export interface Track {
  id: Id;
  name: string;
  kind: TrackKind;
  locked: boolean;
  muted: boolean;
  clips: Clip[];
}

export interface Sequence {
  id: Id;
  name: string;
  width: number;
  height: number;
  frameRate: RationalTime;
  tracks: Track[];
  captions: CaptionSegment[];
  transitions: Transition[];
}

export interface CaptionStyle {
  fontSize: number;
  color: string;
  background: string;
  position: "top" | "center" | "bottom";
}

export interface CaptionSegment {
  id: Id;
  trackId: Id;
  start: RationalTime;
  end: RationalTime;
  text: string;
  style: CaptionStyle;
}

export interface Transition {
  id: Id;
  fromClipId: Id;
  toClipId: Id;
  kind: "cut" | "fade" | "crossDissolve";
  duration: RationalTime;
}

export interface ConversationRecord {
  id: Id;
  provider: AgentProvider;
  title: string;
  externalThreadId?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectDocument {
  schemaVersion: 1;
  id: Id;
  revision: number;
  name: string;
  createdAt: string;
  updatedAt: string;
  media: MediaAsset[];
  sequences: Sequence[];
  activeSequenceId: Id;
  conversations: ConversationRecord[];
  hostedContextConsent: boolean;
  analysisArtifacts: AnalysisArtifact[];
}

export type CommandSource = "manual" | "codex" | "ollama";

export type EditorCommand =
  | { type: "duplicateSequence"; sequenceId: Id; name: string }
  | { type: "setActiveSequence"; sequenceId: Id }
  | { type: "renameSequence"; sequenceId: Id; name: string }
  | { type: "removeSequence"; sequenceId: Id }
  | { type: "setTrackLocked"; trackId: Id; locked: boolean }
  | { type: "setTrackMuted"; trackId: Id; muted: boolean }
  | { type: "addMedia"; asset: MediaAsset }
  | { type: "removeMedia"; assetId: Id }
  | { type: "addClip"; trackId: Id; assetId: Id; timelineStart: RationalTime }
  | { type: "removeClip"; trackId: Id; clipId: Id }
  | { type: "moveClip"; trackId: Id; clipId: Id; timelineStart: RationalTime }
  | { type: "trimClip"; trackId: Id; clipId: Id; sourceIn: RationalTime; sourceOut: RationalTime; timelineStart?: RationalTime }
  | { type: "splitClip"; trackId: Id; clipId: Id; at: RationalTime }
  | { type: "duplicateClip"; trackId: Id; clipId: Id; timelineStart: RationalTime }
  | { type: "changeSpeed"; trackId: Id; clipId: Id; playbackRate: number }
  | { type: "cropClip"; trackId: Id; clipId: Id; transform: Transform }
  | { type: "setOpacity"; trackId: Id; clipId: Id; opacity: number }
  | { type: "setVolume"; trackId: Id; clipId: Id; volume: number }
  | { type: "fadeAudio"; trackId: Id; clipId: Id; fadeIn: RationalTime; fadeOut: RationalTime }
  | { type: "duckAudio"; trackId: Id; clipId: Id; enabled: boolean }
  | { type: "replaceClip"; trackId: Id; clipId: Id; assetId: Id }
  | { type: "addCaption"; trackId: Id; start: RationalTime; end: RationalTime; text: string }
  | { type: "editCaption"; captionId: Id; text: string }
  | { type: "styleCaption"; captionId: Id; style: CaptionStyle }
  | { type: "removeCaption"; captionId: Id }
  | { type: "addTransition"; fromClipId: Id; toClipId: Id; kind: Transition["kind"]; duration: RationalTime }
  | { type: "removeTransition"; transitionId: Id };

export interface ProjectPatch {
  before: ProjectDocument;
  after: ProjectDocument;
}

export interface CommandEnvelope {
  commandId: Id;
  projectId: Id;
  source: CommandSource;
  conversationId?: Id;
  batchId: Id;
  expectedProjectRevision: number;
  payload: EditorCommand;
}

export interface CommandResult {
  newProjectRevision: number;
  affectedEntityIds: Id[];
  forwardPatch: ProjectPatch;
  inversePatch: ProjectPatch;
  warnings: string[];
  jobId?: Id;
}

export interface MediaInspection {
  path: string;
  name: string;
  kind: MediaKind;
  duration: RationalTime;
  width?: number;
  height?: number;
  codec?: string;
  hasAudio: boolean;
  bookmark?: string;
  thumbnailPath?: string;
  waveformPath?: string;
}

export interface ExportRequest {
  outputPath: string;
  width: number;
  height: number;
  frameRate: RationalTime;
  clips: Array<{
    sourcePath: string;
    sourceIn: RationalTime;
    sourceOut: RationalTime;
    playbackRate: number;
  }>;
}

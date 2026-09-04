import { seconds } from "./time";
import type { ProjectDocument } from "../types/project";

const now = new Date().toISOString();

export const sampleProject: ProjectDocument = {
  schemaVersion: 1,
  id: "project-demo",
  revision: 1,
  name: "Summer campaign",
  createdAt: now,
  updatedAt: now,
  hostedContextConsent: false,
  analysisArtifacts: [],
  activeSequenceId: "sequence-main",
  media: [
    { id: "asset-1", name: "Beach walk.mov", kind: "video", path: "media/beach.mov", duration: seconds(7.2), width: 3840, height: 2160, status: "ready", color: "#d98f6f" },
    { id: "asset-2", name: "Product closeup.mov", kind: "video", path: "media/product.mov", duration: seconds(5.8), width: 2160, height: 3840, status: "ready", color: "#7f8ecf" },
    { id: "asset-3", name: "Friends laughing.mov", kind: "video", path: "media/friends.mov", duration: seconds(9.1), width: 3840, height: 2160, status: "analyzing", color: "#6d9c86" },
    { id: "asset-4", name: "Golden hour.mov", kind: "video", path: "media/golden.mov", duration: seconds(8.5), width: 3840, height: 2160, status: "ready", color: "#bb7fa6" },
    { id: "asset-5", name: "Daylight.mp3", kind: "audio", path: "audio/daylight.mp3", duration: seconds(58), status: "ready", color: "#65a48f" },
    { id: "asset-6", name: "Logo.png", kind: "image", path: "graphics/logo.png", duration: seconds(0), width: 1200, height: 1200, status: "ready", color: "#a89973" }
  ],
  sequences: [{
    id: "sequence-main",
    name: "Vertical cut — 20s",
    width: 1080,
    height: 1920,
    frameRate: { value: 30, timescale: 1 },
    tracks: [
      { id: "v1", name: "Video 1", kind: "video", locked: false, muted: false, clips: [
        { id: "clip-1", assetId: "asset-1", name: "Beach walk", sourceIn: seconds(0.5), sourceOut: seconds(4.2), timelineStart: seconds(0), playbackRate: 1, transform: { x: 0, y: 0, scale: 1, rotation: 0, opacity: 1 }, audio: { volume: 1, fadeIn: seconds(0), fadeOut: seconds(0), ducking: false }, color: "#b97258" },
        { id: "clip-2", assetId: "asset-2", name: "Product", sourceIn: seconds(0), sourceOut: seconds(4.1), timelineStart: seconds(3.7), playbackRate: 1, transform: { x: 0, y: 0, scale: 1, rotation: 0, opacity: 1 }, audio: { volume: 1, fadeIn: seconds(0), fadeOut: seconds(0), ducking: false }, color: "#6978bd" },
        { id: "clip-3", assetId: "asset-3", name: "Friends", sourceIn: seconds(1.2), sourceOut: seconds(6.8), timelineStart: seconds(7.8), playbackRate: 1.15, transform: { x: 0, y: 0, scale: 1, rotation: 0, opacity: 1 }, audio: { volume: 1, fadeIn: seconds(0), fadeOut: seconds(0), ducking: false }, color: "#568773" },
        { id: "clip-4", assetId: "asset-4", name: "Golden hour", sourceIn: seconds(2), sourceOut: seconds(7.5), timelineStart: seconds(13.4), playbackRate: 1, transform: { x: 0, y: 0, scale: 1, rotation: 0, opacity: 1 }, audio: { volume: 1, fadeIn: seconds(0), fadeOut: seconds(0), ducking: false }, color: "#9e6589" }
      ]},
      { id: "c1", name: "Captions", kind: "caption", locked: false, muted: false, clips: [] },
      { id: "a1", name: "Music", kind: "audio", locked: false, muted: false, clips: [] }
    ]
  }],
  conversations: [
    { id: "chat-1", provider: "codex", title: "Create the first cut", externalThreadId: "thread-demo", createdAt: now, updatedAt: now },
    { id: "chat-2", provider: "ollama", title: "Alternative opening", createdAt: now, updatedAt: now }
  ]
};

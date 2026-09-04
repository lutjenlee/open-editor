import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { applyEditorCommand, createEnvelope } from "../lib/commands";
import { sampleProject } from "../lib/sampleProject";
import { seconds, toSeconds } from "../lib/time";
import type { CommandEnvelope, CommandResult, EditorCommand, MediaAsset, ProjectDocument, RationalTime } from "../types/project";

interface HistoryEntry { id: string; label: string; before: ProjectDocument; after: ProjectDocument }
export interface RecentProject { name: string; folder: string; openedAt: string; pinned: boolean }

function readRecents(): RecentProject[] {
  try { return JSON.parse(window.localStorage.getItem("open-editor.recents.v1") ?? "[]") as RecentProject[]; }
  catch { return []; }
}

interface EditorState {
  project: ProjectDocument; projectFolder?: string; projectError?: string;
  selectedClipId?: string; selectedAssetId?: string; playhead: RationalTime; isPlaying: boolean;
  projectsOpen: boolean; agentOpen: boolean; timelineOpen: boolean; mediaTab: "all" | "video" | "audio" | "image";
  recentProjects: RecentProject[];
  undoStack: HistoryEntry[]; redoStack: HistoryEntry[];
  replaceProject: (project: ProjectDocument, folder: string) => void; setProjectError: (message?: string) => void;
  selectClip: (id?: string) => void; selectAsset: (id?: string) => void; setPlayhead: (value: RationalTime) => void;
  setPlaying: (playing: boolean) => void; togglePlayback: () => void; toggleProjects: () => void; toggleAgent: () => void; toggleTimeline: () => void;
  setMediaTab: (tab: EditorState["mediaTab"]) => void; dispatch: (command: EditorCommand, label?: string) => Promise<boolean>;
  addMedia: (assets: MediaAsset[]) => Promise<void>; addAssetToTimeline: (assetId: string) => Promise<void>;
  splitSelected: () => Promise<void>; removeSelected: () => Promise<void>; moveSelected: (delta: RationalTime) => Promise<void>; undo: () => Promise<void>; redo: () => Promise<void>;
}

function timelineEnd(project: ProjectDocument): number {
  const sequence = project.sequences.find((item) => item.id === project.activeSequenceId);
  return Math.max(0, ...(sequence?.tracks.flatMap((track) => track.clips.map((clip) =>
    toSeconds(clip.timelineStart) + (toSeconds(clip.sourceOut) - toSeconds(clip.sourceIn)) / clip.playbackRate
  )) ?? [0]));
}

function selectedLocation(project: ProjectDocument, clipId?: string) {
  if (!clipId) return undefined;
  const sequence = project.sequences.find((item) => item.id === project.activeSequenceId);
  for (const track of sequence?.tracks ?? []) {
    const clip = track.clips.find((item) => item.id === clipId);
    if (clip) return { track, clip };
  }
  return undefined;
}

export const useEditorStore = create<EditorState>((set, get) => ({
  project: sampleProject, selectedClipId: "clip-2", selectedAssetId: "asset-2", playhead: seconds(6.4), isPlaying: false,
  projectsOpen: window.localStorage.getItem("open-editor.projects-open.v1") !== "false", agentOpen: false,
  timelineOpen: window.localStorage.getItem("open-editor.timeline-open.v1") !== "false", mediaTab: "all",
  recentProjects: readRecents(),
  undoStack: [], redoStack: [],
  replaceProject: (project, projectFolder) => {
    const recents = [{ name: project.name, folder: projectFolder, openedAt: new Date().toISOString(), pinned: false }, ...get().recentProjects.filter((item) => item.folder !== projectFolder)].slice(0, 12);
    window.localStorage.setItem("open-editor.recents.v1", JSON.stringify(recents));
    set({ project, projectFolder, recentProjects: recents, projectError: undefined, selectedClipId: undefined, selectedAssetId: project.media[0]?.id, playhead: seconds(0), isPlaying: false, undoStack: [], redoStack: [] });
  },
  setProjectError: (projectError) => set({ projectError }), selectClip: (selectedClipId) => set({ selectedClipId }), selectAsset: (selectedAssetId) => set({ selectedAssetId }),
  setPlayhead: (playhead) => set({ playhead }), setPlaying: (isPlaying) => set({ isPlaying }), togglePlayback: () => set((state) => ({ isPlaying: !state.isPlaying })),
  toggleProjects: () => set((state) => { const projectsOpen = !state.projectsOpen; window.localStorage.setItem("open-editor.projects-open.v1", String(projectsOpen)); return { projectsOpen }; }),
  toggleAgent: () => set((state) => ({ agentOpen: !state.agentOpen })),
  toggleTimeline: () => set((state) => { const timelineOpen = !state.timelineOpen; window.localStorage.setItem("open-editor.timeline-open.v1", String(timelineOpen)); return { timelineOpen }; }),
  setMediaTab: (mediaTab) => set({ mediaTab }),
  dispatch: async (command, label = command.type) => {
    const state = get();
    try {
      const envelope = createEnvelope(state.project, command);
      let after: ProjectDocument;
      if (state.projectFolder) {
        const result = await invoke<CommandResult & { project: ProjectDocument }>("dispatch_editor_command", { folder: state.projectFolder, envelope: envelope as CommandEnvelope });
        after = result.project;
      } else {
        after = applyEditorCommand(state.project, envelope).forwardPatch.after;
      }
      const entry = { id: envelope.commandId, label, before: state.project, after };
      set({ project: after, projectError: undefined, redoStack: [], undoStack: [...state.undoStack, entry] });
      return true;
    } catch (error) { set({ projectError: error instanceof Error ? error.message : String(error) }); return false; }
  },
  addMedia: async (assets) => { for (const asset of assets) await get().dispatch({ type: "addMedia", asset }, `Import ${asset.name}`); },
  addAssetToTimeline: async (assetId) => {
    const state = get(); const asset = state.project.media.find((item) => item.id === assetId);
    const sequence = state.project.sequences.find((item) => item.id === state.project.activeSequenceId);
    const track = sequence?.tracks.find((item) => item.kind === (asset?.kind === "audio" ? "audio" : "video"));
    if (!asset || !track) return;
    await get().dispatch({ type: "addClip", trackId: track.id, assetId, timelineStart: seconds(timelineEnd(state.project)) }, `Add ${asset.name}`);
  },
  moveSelected: async (delta) => { const state = get(); const found = selectedLocation(state.project, state.selectedClipId); if (!found) return; await get().dispatch({ type: "moveClip", trackId: found.track.id, clipId: found.clip.id, timelineStart: seconds(Math.max(0, toSeconds(found.clip.timelineStart) + toSeconds(delta))) }, "Move clip"); },
  removeSelected: async () => { const state = get(); const found = selectedLocation(state.project, state.selectedClipId); if (!found) return; if (await get().dispatch({ type: "removeClip", trackId: found.track.id, clipId: found.clip.id }, "Delete clip")) set({ selectedClipId: undefined }); },
  splitSelected: async () => { const state = get(); const found = selectedLocation(state.project, state.selectedClipId); if (!found) return; await get().dispatch({ type: "splitClip", trackId: found.track.id, clipId: found.clip.id, at: state.playhead }, "Split clip"); },
  undo: async () => { const state = get(); const entry = state.undoStack.at(-1); if (!entry) return; const project = { ...structuredClone(entry.before), revision: state.project.revision + 1, updatedAt: new Date().toISOString() }; if (state.projectFolder) await invoke("save_project", { folder: state.projectFolder, project }); set({ project, undoStack: state.undoStack.slice(0, -1), redoStack: [...state.redoStack, entry], selectedClipId: undefined, isPlaying: false }); },
  redo: async () => { const state = get(); const entry = state.redoStack.at(-1); if (!entry) return; const project = { ...structuredClone(entry.after), revision: state.project.revision + 1, updatedAt: new Date().toISOString() }; if (state.projectFolder) await invoke("save_project", { folder: state.projectFolder, project }); set({ project, redoStack: state.redoStack.slice(0, -1), undoStack: [...state.undoStack, entry], selectedClipId: undefined, isPlaying: false }); },
}));

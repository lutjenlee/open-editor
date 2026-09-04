import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ExportRequest, MediaInspection, ProjectDocument } from "../types/project";

export function isDesktop(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export interface CommandServiceInfo { socketPath: string; capabilityToken: string }
export interface RecentProject { name: string; folder: string; openedAt: string; pinned: boolean }

export async function authorizeCommandProject(folder: string): Promise<CommandServiceInfo> {
  return invoke<CommandServiceInfo>("authorize_command_project", { folder });
}

async function chooseFolder(): Promise<string | null> {
  const selection = await open({ directory: true, multiple: false, canCreateDirectories: true });
  return typeof selection === "string" ? selection : null;
}

export async function createProjectFolder(): Promise<{ folder: string; project: ProjectDocument } | null> {
  if (!isDesktop()) return null;
  const folder = await chooseFolder();
  if (!folder) return null;
  const fallbackName = folder.split("/").filter(Boolean).at(-1) ?? "Untitled project";
  const project = await invoke<ProjectDocument>("create_project", { folder, name: fallbackName });
  await authorizeCommandProject(folder);
  return { folder, project };
}

export async function openProjectFolder(): Promise<{ folder: string; project: ProjectDocument } | null> {
  if (!isDesktop()) return null;
  const folder = await chooseFolder();
  if (!folder) return null;
  const project = await invoke<ProjectDocument>("open_project", { folder });
  await authorizeCommandProject(folder);
  return { folder, project };
}

export async function openProjectAtPath(folder: string): Promise<{ folder: string; project: ProjectDocument }> {
  if (!isDesktop()) throw new Error("Folder projects require the desktop app.");
  const project = await invoke<ProjectDocument>("open_project", { folder });
  await authorizeCommandProject(folder);
  return { folder, project };
}

export async function saveProjectFolder(folder: string, project: ProjectDocument): Promise<void> {
  if (!isDesktop()) return;
  await invoke("save_project", { folder, project });
}

export async function importMediaFiles(projectFolder: string): Promise<MediaInspection[]> {
  if (!isDesktop()) return [];
  const selection = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Media", extensions: ["mov", "mp4", "m4v", "webm", "jpg", "jpeg", "png", "heic", "wav", "mp3", "m4a", "aac"] }],
  });
  const paths = typeof selection === "string" ? [selection] : selection ?? [];
  if (paths.length === 0) return [];
  return inspectMediaPaths(projectFolder, paths);
}

export async function inspectMediaPaths(projectFolder: string, paths: string[]): Promise<MediaInspection[]> {
  if (!isDesktop() || paths.length === 0) return [];
  return invoke<MediaInspection[]>("inspect_media", { paths, projectFolder });
}

export async function relinkMediaFile(projectFolder: string, assetId: string): Promise<ProjectDocument | null> {
  if (!isDesktop()) return null;
  const selection = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Media", extensions: ["mov", "mp4", "m4v", "webm", "jpg", "jpeg", "png", "heic", "wav", "mp3", "m4a", "aac"] }],
  });
  if (typeof selection !== "string") return null;
  const [inspection] = await invoke<MediaInspection[]>("inspect_media", { paths: [selection], projectFolder });
  return invoke<ProjectDocument>("relink_media", { folder: projectFolder, assetId, inspection });
}

export async function chooseExportPath(defaultName: string): Promise<string | null> {
  if (!isDesktop()) return null;
  return save({ defaultPath: `${defaultName}.mp4`, filters: [{ name: "MPEG-4 Video", extensions: ["mp4"] }] });
}

export async function exportVideo(request: ExportRequest): Promise<string> {
  return invoke<string>("export_video", { request });
}

export async function createMediaProxy(folder: string, assetId: string): Promise<ProjectDocument> {
  return invoke<ProjectDocument>("create_media_proxy", { folder, assetId });
}

export async function analyzeMediaAsset(folder: string, assetId: string): Promise<ProjectDocument> {
  return invoke<ProjectDocument>("analyze_media_asset", { folder, assetId });
}

export interface MediaJobRecord {
  id: string;
  kind: "proxy" | "analysis" | "export" | "transcription" | "modelDownload";
  assetId?: string;
  status: "queued" | "running" | "cancelling" | "cancelled" | "completed" | "failed";
  progress: number;
  message: string;
  createdAt: string;
  updatedAt: string;
  result?: ProjectDocument | string;
  error?: string;
}

export interface TranscriptionStatus {
  engineInstalled: boolean;
  modelInstalled: boolean;
  modelPath: string;
  modelName: string;
  downloadSizeBytes: number;
  license: string;
  fullyLocalAfterInstall: boolean;
}

export async function getTranscriptionStatus(): Promise<TranscriptionStatus> {
  return invoke<TranscriptionStatus>("transcription_status");
}

export async function startTranscriptionModelDownload(): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("start_transcription_model_download");
}

export async function deleteTranscriptionModel(): Promise<void> {
  return invoke("delete_transcription_model");
}

export async function startTranscriptionJob(folder: string, assetId: string): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("start_transcription_job", { folder, assetId });
}

export async function startMediaJob(folder: string, assetId: string, kind: "proxy" | "analysis"): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("start_media_job", { folder, assetId, kind });
}

export async function startExportJob(request: ExportRequest): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("start_export_job", { request });
}

export async function startNativeExportJob(folder: string, outputPath: string): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("start_native_export_job", { folder, outputPath });
}

export async function cancelMediaJob(jobId: string): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("cancel_media_job", { jobId });
}

export async function getMediaJob(jobId: string): Promise<MediaJobRecord> {
  return invoke<MediaJobRecord>("get_media_job", { jobId });
}

export async function listRecentProjects(): Promise<RecentProject[]> {
  return isDesktop() ? invoke<RecentProject[]>("list_recent_projects") : [];
}

export async function rememberRecentProject(name: string, folder: string): Promise<RecentProject[]> {
  return invoke<RecentProject[]>("remember_recent_project", { name, folder });
}

export async function setRecentProjectPinned(folder: string, pinned: boolean): Promise<RecentProject[]> {
  return invoke<RecentProject[]>("set_recent_project_pinned", { folder, pinned });
}

export async function removeRecentProject(folder: string): Promise<RecentProject[]> {
  return invoke<RecentProject[]>("remove_recent_project", { folder });
}

export async function revealProject(folder: string): Promise<void> {
  return invoke("reveal_project", { folder });
}

export interface NativePlaybackState { value: number; timescale: number; rate: number }

export async function attachNativePreview(frame: [number, number, number, number]): Promise<void> {
  return invoke("native_preview_attach", { frame });
}

export async function setNativePreviewFrame(frame: [number, number, number, number]): Promise<void> {
  return invoke("native_preview_set_frame", { frame });
}

export async function loadNativePreview(folder: string): Promise<void> {
  return invoke("native_preview_load", { folder });
}

export async function controlNativePreview(action: "play" | "pause" | "seek" | "detach" | "status", value?: number, timescale = 600): Promise<NativePlaybackState> {
  return invoke<NativePlaybackState>("native_preview_control", { action, value, timescale });
}

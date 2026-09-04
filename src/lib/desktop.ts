import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ExportRequest, MediaInspection, ProjectDocument } from "../types/project";

export function isDesktop(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export interface CommandServiceInfo { socketPath: string; capabilityToken: string }

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

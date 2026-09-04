import {
  ChevronDown, ChevronLeft, ChevronRight, CircleUserRound,
  Download, FilePlus2, Files, Film, Folder, Gauge, Image, Library, Lock, LogOut, MessageCircle, MessageSquarePlus, Mic2, MoreHorizontal,
  Music2, PanelBottom, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, Pause, Play, Plus, Redo2, Scissors,
  Search, Send, Settings2, Share2, SkipBack, SkipForward, Trash2, Undo2, Upload, UserPlus, Volume2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { formatTimecode, seconds, toSeconds } from "./lib/time";
import { analyzeMediaAsset, chooseExportPath, createMediaProxy, createProjectFolder, exportVideo, importMediaFiles, isDesktop, openProjectAtPath, openProjectFolder, relinkMediaFile, saveProjectFolder } from "./lib/desktop";
import { useEditorStore } from "./store/editorStore";
import type { MediaAsset, Track } from "./types/project";

function IconButton({ label, children, onClick, active = false, disabled = false }: { label: string; children: React.ReactNode; onClick?: () => void; active?: boolean; disabled?: boolean }) {
  return <button className={`icon-button ${active ? "active" : ""}`} aria-label={label} title={label} onClick={onClick} disabled={disabled}>{children}</button>;
}

function ProjectSidebar({ collapsed }: { collapsed: boolean }) {
  const project = useEditorStore((s) => s.project);
  const replaceProject = useEditorStore((s) => s.replaceProject);
  const setProjectError = useEditorStore((s) => s.setProjectError);
  const recentProjects = useEditorStore((s) => s.recentProjects);
  const [expandedProjects, setExpandedProjects] = useState(() => new Set([project.name]));
  const [accountMenuOpen, setAccountMenuOpen] = useState(false);
  const accountMenuRef = useRef<HTMLDivElement>(null);
  const toggleProject = (name: string) => setExpandedProjects((current) => {
    const next = new Set(current);
    if (next.has(name)) next.delete(name); else next.add(name);
    return next;
  });
  useEffect(() => {
    if (!accountMenuOpen) return;
    const closeMenu = (event: MouseEvent) => {
      if (!accountMenuRef.current?.contains(event.target as Node)) setAccountMenuOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAccountMenuOpen(false);
    };
    document.addEventListener("mousedown", closeMenu);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeMenu);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [accountMenuOpen]);
  const chooseProject = async (mode: "create" | "open") => {
    if (!isDesktop()) {
      setProjectError("Folder projects are available in the Tauri desktop app. This browser build is a UI preview.");
      return;
    }
    try {
      const result = mode === "create" ? await createProjectFolder() : await openProjectFolder();
      if (result) replaceProject(result.project, result.folder);
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : String(error));
    }
  };
  const openRecent = async (folder: string) => {
    try { const result = await openProjectAtPath(folder); replaceProject(result.project, result.folder); }
    catch (error) { setProjectError(`Could not open recent project: ${error instanceof Error ? error.message : String(error)}`); }
  };
  return <aside className="project-sidebar" aria-hidden={collapsed} inert={collapsed}>
    <div className="sidebar-window-drag-region" data-tauri-drag-region />
    <div className="brand-row"><strong>Open Editor</strong><ChevronDown size={13} /><span className="brand-spacer" /><IconButton label="Sidebar options"><MoreHorizontal size={15} /></IconButton></div>
    <div className="sidebar-actions">
      <button className="primary-action" onClick={() => void chooseProject("create")}><FilePlus2 size={15} /> New project</button>
      <button className="nav-action" onClick={() => void chooseProject("open")}><Files size={15} /> Open folder</button>
    </div>
    <div className="sidebar-heading"><span>Pinned</span></div>
    <div className="pinned-list">{recentProjects.filter((item) => item.pinned).map((item) => <button key={item.folder} className="sidebar-project-link" onClick={() => void openRecent(item.folder)}><span>{item.name}</span></button>)}{!recentProjects.some((item) => item.pinned) && <span className="sidebar-empty">No pinned projects</span>}</div>
    <div className="sidebar-heading"><span>Projects</span><Search size={13} /></div>
    <div className="project-list">
      <div className={`project-item ${expandedProjects.has(project.name) ? "expanded" : ""}`}>
        <button className="project-name" onClick={() => toggleProject(project.name)} aria-expanded={expandedProjects.has(project.name)}>
          <Folder size={14} /><span>{project.name}</span><ChevronRight className="project-chevron" size={14} />
        </button>
        {expandedProjects.has(project.name) && <div className="conversation-list">
          {project.conversations.map((chat, chatIndex) => <button className={chatIndex === 0 ? "selected" : ""} key={chat.id}>{chat.title}</button>)}
        </div>}
      </div>
      {recentProjects.filter((item) => item.name !== project.name).map((item) => <div key={item.folder} className={`project-item ${expandedProjects.has(item.name) ? "expanded" : ""}`}>
        <button className="project-name" onClick={() => toggleProject(item.name)} aria-expanded={expandedProjects.has(item.name)}>
          <Folder size={14} /><span>{item.name}</span><ChevronRight className="project-chevron" size={14} />
        </button>
        {expandedProjects.has(item.name) && <div className="conversation-list"><button onClick={() => void openRecent(item.folder)}>Open project</button></div>}
      </div>)}
    </div>
    <div className="sidebar-footer" ref={accountMenuRef}>
      {accountMenuOpen && <div className="account-menu" role="menu">
        <div className="account-menu-profile"><CircleUserRound size={25} /><span><strong>Codex</strong><small>Connect account</small></span></div>
        <div className="account-menu-divider" />
        <button role="menuitem"><Gauge size={16} /><span>Usage</span><small>View limits</small></button>
        <button role="menuitem"><UserPlus size={16} /><span>Invite a friend</span></button>
        <button role="menuitem" onClick={() => setAccountMenuOpen(false)}><Settings2 size={16} /><span>Preferences</span></button>
        <div className="account-menu-divider" />
        <button role="menuitem" className="account-menu-muted"><LogOut size={16} /><span>Sign out</span></button>
      </div>}
      <div className="account-row">
        <button className="account-button"><CircleUserRound size={17} /><span><strong>Codex</strong><small>Connect account</small></span></button>
        <IconButton label="Account options" active={accountMenuOpen} onClick={() => setAccountMenuOpen((open) => !open)}><MoreHorizontal size={16} /></IconButton>
      </div>
    </div>
  </aside>;
}

function MediaCard({ asset }: { asset: MediaAsset }) {
  const selected = useEditorStore((s) => s.selectedAssetId === asset.id);
  const select = useEditorStore((s) => s.selectAsset);
  const add = useEditorStore((s) => s.addAssetToTimeline);
  const icon = asset.kind === "video" ? <Film size={14} /> : asset.kind === "audio" ? <Music2 size={14} /> : <Image size={14} />;
  return <button className={`media-card ${selected ? "selected" : ""}`} onClick={() => select(asset.id)} onDoubleClick={() => add(asset.id)} title="Select; double-click to add to the timeline">
    <div className="media-thumb" style={{ "--media-color": asset.color } as React.CSSProperties}>
      {asset.thumbnailPath && <img src={convertFileSrc(asset.thumbnailPath)} alt="" />}
      <span className="media-shape" />
      <span className="duration">{asset.kind === "image" ? "STILL" : `${toSeconds(asset.duration).toFixed(1)}s`}</span>
      {asset.status === "analyzing" && <span className="analysis-dot" />}
    </div>
    <span className="media-name">{icon}{asset.name}</span>
  </button>;
}

function MediaLibrary() {
  const project = useEditorStore((s) => s.project);
  const tab = useEditorStore((s) => s.mediaTab);
  const setTab = useEditorStore((s) => s.setMediaTab);
  const projectFolder = useEditorStore((s) => s.projectFolder);
  const addMedia = useEditorStore((s) => s.addMedia);
  const setError = useEditorStore((s) => s.setProjectError);
  const selectedAssetId = useEditorStore((s) => s.selectedAssetId);
  const updateProject = useEditorStore((s) => s.updateProject);
  const [importing, setImporting] = useState(false);
  const [preparing, setPreparing] = useState<"proxy" | "analysis">();
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filtered = project.media.filter((asset) =>
    (tab === "all" || asset.kind === tab) &&
    (!normalizedQuery || asset.name.toLocaleLowerCase().includes(normalizedQuery))
  );
  const importFiles = async () => {
    if (!projectFolder) { setError("Create or open a folder-backed project before importing media."); return; }
    setImporting(true);
    try {
      const inspected = await importMediaFiles(projectFolder);
      addMedia(inspected.map((item, index) => ({ ...item, id: crypto.randomUUID(), status: "ready" as const, color: ["#b97258", "#6978bd", "#568773", "#9e6589", "#3b8067"][index % 5] })));
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { setImporting(false); }
  };
  const prepareSelected = async (kind: "proxy" | "analysis") => {
    if (!projectFolder || !selectedAssetId) return;
    setPreparing(kind);
    try {
      const next = kind === "proxy"
        ? await createMediaProxy(projectFolder, selectedAssetId)
        : await analyzeMediaAsset(projectFolder, selectedAssetId);
      updateProject(next);
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { setPreparing(undefined); }
  };
  const relinkSelected = async () => {
    if (!projectFolder || !selectedAssetId) return;
    setPreparing("analysis");
    try {
      const next = await relinkMediaFile(projectFolder, selectedAssetId);
      if (next) updateProject(next);
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { setPreparing(undefined); }
  };
  const selectedAsset = project.media.find((asset) => asset.id === selectedAssetId);
  return <section className="media-library panel">
    <div className="panel-title"><div><Library size={15} /><strong>Media</strong></div><IconButton label="Import media" onClick={() => void importFiles()} disabled={importing}><Upload size={15} /></IconButton></div>
    <div className="media-tabs">
      {(["all", "video", "audio", "image"] as const).map((item) => <button key={item} className={tab === item ? "active" : ""} onClick={() => setTab(item)}>{item}</button>)}
    </div>
    <div className="media-search"><Search size={13} /><input aria-label="Search media" placeholder="Search media" value={query} onChange={(event) => setQuery(event.target.value)} /></div>
    {selectedAssetId && <div className="media-preparation-actions">
      {selectedAsset?.status === "missing" ? <button onClick={() => void relinkSelected()} disabled={Boolean(preparing)}><Files size={13} />Relink</button> : <>
        <button onClick={() => void prepareSelected("analysis")} disabled={Boolean(preparing)}><Gauge size={13} />{preparing === "analysis" ? "Analyzing…" : "Analyze"}</button>
        {selectedAsset?.kind === "video" && <button onClick={() => void prepareSelected("proxy")} disabled={Boolean(preparing)}><Film size={13} />{preparing === "proxy" ? "Creating…" : "Make proxy"}</button>}
      </>}
    </div>}
    <div className="media-grid">{filtered.map((asset) => <MediaCard key={asset.id} asset={asset} />)}{filtered.length === 0 && <p className="media-empty">No matching media</p>}</div>
    <button className="import-drop" onClick={() => void importFiles()} disabled={importing}><Plus size={14} /> {importing ? "Inspecting…" : "Import media"}</button>
  </section>;
}

function Viewer() {
  const playing = useEditorStore((s) => s.isPlaying);
  const toggle = useEditorStore((s) => s.togglePlayback);
  const playhead = useEditorStore((s) => s.playhead);
  const setPlayhead = useEditorStore((s) => s.setPlayhead);
  const setPlaying = useEditorStore((s) => s.setPlaying);
  const project = useEditorStore((s) => s.project);
  const selectedAssetId = useEditorStore((s) => s.selectedAssetId);
  const selectedClipId = useEditorStore((s) => s.selectedClipId);
  const videoRef = useRef<HTMLVideoElement>(null);
  const sequence = project.sequences.find((item) => item.id === project.activeSequenceId);
  const selectedClip = sequence?.tracks.flatMap((track) => track.clips).find((clip) => clip.id === selectedClipId);
  const asset = project.media.find((item) => item.id === (selectedClip?.assetId ?? selectedAssetId));
  const duration = Math.max(1, ...(sequence?.tracks.flatMap((track) => track.clips.map((clip) => toSeconds(clip.timelineStart) + (toSeconds(clip.sourceOut) - toSeconds(clip.sourceIn)) / clip.playbackRate)) ?? [toSeconds(asset?.duration ?? seconds(1))]));
  useEffect(() => { const video = videoRef.current; if (!video) return; if (playing) void video.play().catch(() => setPlaying(false)); else video.pause(); }, [playing, setPlaying, asset?.id]);
  const seek = (value: number) => { setPlayhead(seconds(value)); if (videoRef.current && selectedClip) videoRef.current.currentTime = toSeconds(selectedClip.sourceIn) + Math.max(0, value - toSeconds(selectedClip.timelineStart)) * selectedClip.playbackRate; };
  return <section className="viewer panel">
    <div className="viewer-toolbar"><span>{sequence?.name ?? "Sequence"}</span><div><button>Fit <ChevronDown size={12} /></button><IconButton label="Viewer settings"><Settings2 size={14} /></IconButton></div></div>
    <div className="viewer-stage">
      {asset?.kind === "video" && isDesktop() ? <video ref={videoRef} className="native-video" src={convertFileSrc(asset.proxyPath ?? asset.path)} onEnded={() => setPlaying(false)} onTimeUpdate={(event) => { if (!selectedClip) return; const position = toSeconds(selectedClip.timelineStart) + Math.max(0, event.currentTarget.currentTime - toSeconds(selectedClip.sourceIn)) / selectedClip.playbackRate; setPlayhead(seconds(position)); }} /> : asset?.kind === "image" && isDesktop() ? <img className="native-video" src={convertFileSrc(asset.path)} alt={asset.name} /> : <div className="portrait-video">
        <div className="sun" /><div className="horizon" /><div className="subject"><span /></div>
        <div className="caption-preview">Make every second count.</div>
      </div>}
      <span className="preview-badge">{asset ? `${asset.proxyPath ? "PROXY" : asset.codec ?? asset.kind} · ${asset.width ?? "—"}×${asset.height ?? "—"}` : "NO MEDIA"}</span>
    </div>
    <div className="transport">
      <IconButton label="Previous frame (preview not connected)" disabled><SkipBack size={15} /></IconButton>
      <button className="play-button" aria-label={playing ? "Pause" : "Play"} onClick={toggle}>{playing ? <Pause size={16} fill="currentColor" /> : <Play size={16} fill="currentColor" />}</button>
      <IconButton label="Next frame (preview not connected)" disabled><SkipForward size={15} /></IconButton>
      <span className="timecode">{formatTimecode(playhead)}</span>
      <input className="scrubber" aria-label="Playhead" type="range" min="0" max={duration} step="0.033" value={Math.min(duration, toSeconds(playhead))} onChange={(event) => seek(Number(event.target.value))} />
      <span className="timecode muted">{formatTimecode(seconds(duration))}</span>
      <Volume2 size={15} />
    </div>
  </section>;
}

function TrackRow({ track, pixelsPerSecond, sequence }: { track: Track; pixelsPerSecond: number; sequence: import("./types/project").Sequence }) {
  const selected = useEditorStore((s) => s.selectedClipId);
  const select = useEditorStore((s) => s.selectClip);
  return <div className="track-row">
    <div className="track-label"><span>{track.name}</span><div><Lock size={11} /><Volume2 size={11} /></div></div>
    <div className={`track-lane ${track.kind}`}>
      {track.clips.map((clip) => {
        const start = toSeconds(clip.timelineStart);
        const duration = (toSeconds(clip.sourceOut) - toSeconds(clip.sourceIn)) / clip.playbackRate;
        return <button key={clip.id} onClick={() => select(clip.id)} className={`timeline-clip ${selected === clip.id ? "selected" : ""}`} style={{ left: start * pixelsPerSecond, width: Math.max(52, duration * pixelsPerSecond), background: clip.color }}>
          <span className="clip-filmstrip" /><span>{clip.name}</span><small>{duration.toFixed(1)}s</small>
        </button>;
      })}
      {track.kind === "caption" && sequence.captions.filter((caption) => caption.trackId === track.id).map((caption) => <div key={caption.id} className="caption-block" style={{ left: toSeconds(caption.start) * pixelsPerSecond, width: Math.max(52, (toSeconds(caption.end) - toSeconds(caption.start)) * pixelsPerSecond) }}>{caption.text}</div>)}
    </div>
  </div>;
}

function Timeline() {
  const project = useEditorStore((s) => s.project);
  const playhead = useEditorStore((s) => s.playhead);
  const split = useEditorStore((s) => s.splitSelected);
  const remove = useEditorStore((s) => s.removeSelected);
  const move = useEditorStore((s) => s.moveSelected);
  const undo = useEditorStore((s) => s.undo);
  const redo = useEditorStore((s) => s.redo);
  const duplicate = useEditorStore((s) => s.duplicateSelected);
  const dispatch = useEditorStore((s) => s.dispatch);
  const selectedClipId = useEditorStore((s) => s.selectedClipId);
  const canUndo = useEditorStore((s) => s.undoStack.length > 0);
  const canRedo = useEditorStore((s) => s.redoStack.length > 0);
  const sequence = project.sequences.find((item) => item.id === project.activeSequenceId)!;
  const px = 42;
  const selected = sequence.tracks.flatMap((track) => track.clips.map((clip) => ({ track, clip }))).find((item) => item.clip.id === selectedClipId);
  const addCaption = () => {
    const track = sequence.tracks.find((item) => item.kind === "caption");
    if (track) void dispatch({ type: "addCaption", trackId: track.id, start: playhead, end: seconds(toSeconds(playhead) + 2), text: "New caption" }, "Add caption");
  };
  return <section className="timeline panel">
    <div className="timeline-toolbar">
      <div><strong>Timeline</strong><span className="sequence-pill">9:16 · 1080 × 1920</span></div>
      <div className="timeline-tools"><IconButton label="Undo" onClick={undo} disabled={!canUndo}><Undo2 size={14} /></IconButton><IconButton label="Redo" onClick={redo} disabled={!canRedo}><Redo2 size={14} /></IconButton><span className="toolbar-divider" /><IconButton label="Add caption" onClick={addCaption}><MessageSquarePlus size={14} /></IconButton><IconButton label="Move left" onClick={() => move(seconds(-0.25))} disabled={!selected}><ChevronLeft size={14} /></IconButton><IconButton label="Split clip" onClick={split} disabled={!selected}><Scissors size={14} /></IconButton><IconButton label="Duplicate clip" onClick={duplicate} disabled={!selected}><Files size={14} /></IconButton><IconButton label="Move right" onClick={() => move(seconds(0.25))} disabled={!selected}><ChevronRight size={14} /></IconButton><IconButton label="Delete clip" onClick={remove} disabled={!selected}><Trash2 size={14} /></IconButton></div>
    </div>
    {selected && <div className="clip-inspector" aria-label="Selected clip controls">
      <span>{selected.clip.name}</span>
      <label>Speed<select value={selected.clip.playbackRate} onChange={(event) => void dispatch({ type: "changeSpeed", trackId: selected.track.id, clipId: selected.clip.id, playbackRate: Number(event.target.value) }, "Change speed")}><option value="0.5">0.5×</option><option value="1">1×</option><option value="1.5">1.5×</option><option value="2">2×</option></select></label>
      <label>Opacity<input type="range" min="0" max="1" step="0.05" value={selected.clip.transform.opacity} onChange={(event) => void dispatch({ type: "setOpacity", trackId: selected.track.id, clipId: selected.clip.id, opacity: Number(event.target.value) }, "Set opacity")} /></label>
      <label>Volume<input type="range" min="0" max="2" step="0.05" value={selected.clip.audio.volume} onChange={(event) => void dispatch({ type: "setVolume", trackId: selected.track.id, clipId: selected.clip.id, volume: Number(event.target.value) }, "Set volume")} /></label>
      <label className="check-control"><input type="checkbox" checked={selected.clip.audio.ducking} onChange={(event) => void dispatch({ type: "duckAudio", trackId: selected.track.id, clipId: selected.clip.id, enabled: event.target.checked }, "Toggle ducking")} />Duck</label>
    </div>}
    <div className="timeline-scroll">
      <div className="ruler"><div className="ruler-spacer" />{Array.from({ length: 21 }).map((_, index) => <span key={index} style={{ left: 112 + index * px }}>{index % 5 === 0 ? `00:${String(index).padStart(2, "0")}` : "·"}</span>)}</div>
      <div className="tracks-wrap">
        <div className="playhead-line" style={{ left: 112 + toSeconds(playhead) * px }}><span /></div>
        {sequence.tracks.map((track) => <TrackRow key={track.id} track={track} pixelsPerSecond={px} sequence={sequence} />)}
      </div>
    </div>
  </section>;
}

function AgentPanel({ onClose }: { onClose: () => void }) {
  const [activeTab, setActiveTab] = useState<"chat" | "activity">("chat");
  const [draft, setDraft] = useState("");
  return <aside className="agent-panel panel">
    <div className="chat-header">
      <div className="chat-tabs" role="tablist" aria-label="Chat sidebar">
        <button role="tab" aria-selected={activeTab === "chat"} className={activeTab === "chat" ? "active" : ""} onClick={() => setActiveTab("chat")}><MessageCircle size={13} /> Chat</button>
        <button role="tab" aria-selected={activeTab === "activity"} className={activeTab === "activity" ? "active" : ""} onClick={() => setActiveTab("activity")}>Activity</button>
      </div>
      <div className="chat-header-actions"><IconButton label="New chat" onClick={() => { setActiveTab("chat"); setDraft(""); }}><MessageSquarePlus size={15} /></IconButton><IconButton label="More chat options"><MoreHorizontal size={16} /></IconButton><IconButton label="Close chat sidebar" onClick={onClose}><PanelRightClose size={16} /></IconButton></div>
    </div>
    {activeTab === "chat" ? <div className="messages" role="tabpanel">
      <div className="message user"><p>Tighten the opening and match the cut to the beat.</p></div>
      <div className="message assistant"><p>I’ll review the first sequence and prepare a tighter opening.</p><div className="tool-summary"><span><Film size={12} /> Reading timeline</span><span>3 clips checked · ready to edit</span></div></div>
    </div> : <div className="activity-panel" role="tabpanel">
      <div><span className="activity-dot" /><p><strong>Timeline analyzed</strong><small>3 clips and 1 audio track</small></p></div>
      <div><span className="activity-dot complete" /><p><strong>Media indexed</strong><small>Ready</small></p></div>
    </div>}
    <form className="composer" onSubmit={(event) => { event.preventDefault(); setDraft(""); }}>
      <textarea aria-label="Chat message" placeholder="Plan, build, or ask about this edit" value={draft} onChange={(event) => setDraft(event.target.value)} />
      <div><div className="composer-tools"><button type="button" aria-label="Add context"><Plus size={15} /></button><select aria-label="Model"><option>Codex</option><option>Local model</option></select></div><div className="composer-tools"><button type="button" aria-label="Voice input"><Mic2 size={15} /></button><button className="send-button" type="submit" aria-label="Send message" disabled={!draft.trim()}><Send size={14} /></button></div></div>
    </form>
  </aside>;
}

export default function App() {
  const projectsOpen = useEditorStore((s) => s.projectsOpen);
  const agentOpen = useEditorStore((s) => s.agentOpen);
  const timelineOpen = useEditorStore((s) => s.timelineOpen);
  const toggleProjects = useEditorStore((s) => s.toggleProjects);
  const toggleAgent = useEditorStore((s) => s.toggleAgent);
  const toggleTimeline = useEditorStore((s) => s.toggleTimeline);
  const project = useEditorStore((s) => s.project);
  const projectFolder = useEditorStore((s) => s.projectFolder);
  const projectError = useEditorStore((s) => s.projectError);
  const setProjectError = useEditorStore((s) => s.setProjectError);
  const [saveState, setSaveState] = useState<"demo" | "saving" | "saved" | "error">(projectFolder ? "saved" : "demo");
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  const saveRequest = useRef(0);
  const [exportState, setExportState] = useState<"idle" | "exporting">("idle");
  const saveLabel = saveState === "demo" ? "Demo · not saved" : saveState === "saving" ? "Saving…" : saveState === "error" ? "Save failed" : "Saved";

  useEffect(() => {
    if (!projectFolder) {
      setSaveState("demo");
      return;
    }
    const request = ++saveRequest.current;
    setSaveState("saving");
    const timer = window.setTimeout(() => {
      saveQueue.current = saveQueue.current
        .catch(() => undefined)
        .then(() => saveProjectFolder(projectFolder, project));
      void saveQueue.current.then(() => {
        if (request === saveRequest.current) setSaveState("saved");
      }).catch((error) => {
        if (request !== saveRequest.current) return;
        setSaveState("error");
        setProjectError(`Autosave failed: ${error instanceof Error ? error.message : String(error)}`);
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [project, projectFolder, setProjectError]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey && event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleProjects();
      } else if (event.metaKey && event.key.toLowerCase() === "j") {
        event.preventDefault();
        toggleTimeline();
      } else if (event.metaKey && event.shiftKey && event.key.toLowerCase() === "i") {
        event.preventDefault();
        toggleAgent();
      } else if (event.metaKey && event.key.toLowerCase() === "z") { event.preventDefault(); if (event.shiftKey) useEditorStore.getState().redo(); else useEditorStore.getState().undo(); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [toggleAgent, toggleProjects, toggleTimeline]);

  const startExport = async () => {
    if (!projectFolder) { setProjectError("Create or open a folder-backed project before exporting."); return; }
    const sequence = project.sequences.find((item) => item.id === project.activeSequenceId);
    if (!sequence) return;
    const assets = new Map(project.media.map((asset) => [asset.id, asset]));
    const clips = sequence.tracks.filter((track) => track.kind === "video").flatMap((track) => track.clips).sort((a, b) => toSeconds(a.timelineStart) - toSeconds(b.timelineStart)).map((clip) => ({ clip, asset: assets.get(clip.assetId) })).filter((item) => item.asset?.kind === "video");
    if (clips.length === 0) { setProjectError("Add at least one imported video clip before exporting."); return; }
    const outputPath = await chooseExportPath(project.name); if (!outputPath) return;
    setExportState("exporting");
    try {
      await exportVideo({ outputPath, width: sequence.width, height: sequence.height, frameRate: sequence.frameRate, clips: clips.map(({ clip, asset }) => ({ sourcePath: asset!.path, sourceIn: clip.sourceIn, sourceOut: clip.sourceOut, playbackRate: clip.playbackRate })) });
      setProjectError(`Export complete: ${outputPath}`);
    } catch (error) { setProjectError(`Export failed: ${error instanceof Error ? error.message : String(error)}`); }
    finally { setExportState("idle"); }
  };

  return <main className={`app-shell ${projectsOpen ? "projects-open" : ""} ${agentOpen ? "agent-open" : ""} ${timelineOpen ? "timeline-open" : "timeline-closed"}`}>
    <ProjectSidebar collapsed={!projectsOpen} />
    <button
      className="sidebar-toggle"
      aria-label={projectsOpen ? "Close project sidebar" : "Open project sidebar"}
      aria-pressed={projectsOpen}
      data-tooltip="Toggle sidebar"
      data-shortcut="⌘B"
      onClick={toggleProjects}
    >
      {projectsOpen ? <PanelLeftClose size={16} /> : <PanelLeftOpen size={16} />}
    </button>
    <div className="workspace">
      <header className="titlebar" data-tauri-drag-region>
        <div><Folder className="header-project-icon" size={15} /><span className="project-breadcrumb">{project.name}</span><span className={`save-state ${saveState}`} aria-live="polite">{saveLabel}</span></div>
        <div className="title-actions"><button className="share-button" onClick={() => setProjectError("Sharing will be available when this local project is connected.")}><Share2 size={14} /> Share</button><button className="export-button" onClick={() => void startExport()} disabled={exportState === "exporting"}><Download size={14} /> {exportState === "exporting" ? "Exporting…" : "Export"}</button><IconButton label={`${timelineOpen ? "Close" : "Open"} timeline (⌘J)`} active={timelineOpen} onClick={toggleTimeline}><PanelBottom size={16} /></IconButton><IconButton label={`${agentOpen ? "Close" : "Open"} chat sidebar (⌘⇧I)`} active={agentOpen} onClick={toggleAgent}>{agentOpen ? <PanelRightClose size={16} /> : <PanelRightOpen size={16} />}</IconButton></div>
      </header>
      {projectError && <div className="app-notice" role="alert"><span>{projectError}</span><button onClick={() => setProjectError(undefined)}>Dismiss</button></div>}
      <div className="editor-grid"><MediaLibrary /><Viewer /><Timeline /></div>
    </div>
    {agentOpen && <AgentPanel onClose={toggleAgent} />}
  </main>;
}

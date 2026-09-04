# Architecture

## Trust boundaries

React renders project snapshots and dispatches manual intents. Folder-backed mutations cross the Tauri boundary and are revalidated, revision-checked, persisted, and audited by the Rust command dispatcher. The in-memory TypeScript implementation supports the non-persistent UI demo and is contract-tested against the same envelope shape. A narrow C-compatible Swift boundary is reserved for AVFoundation composition playback. Neither a hosted nor local model receives direct write access to project files.

```text
React UI
   │ validated Tauri commands + events
Rust project engine ───────── Swift AVFoundation bridge
   │
   ├── FFmpeg / FFprobe / whisper.cpp workers
   ├── Codex App Server ── local MCP adapter ── command dispatcher
   └── Ollama loopback API ────────────────┘
```

Every manual or agent edit becomes an `EditorCommand` envelope containing a command, source, conversation and batch IDs, and expected project revision. The dispatcher validates it, applies it atomically, records forward and inverse patches, autosaves, and publishes a new immutable snapshot.

## Portable projects

`open-editor.project.json` is the versioned, human-readable source of truth. The hidden `.open-editor` directory holds history, chats, and reconstructable caches. In-project media uses relative paths. External media stores a display path plus a macOS security-scoped bookmark; an invalid or missing file is surfaced for explicit relinking.

## Agent isolation

The Codex App Server runs as a pinned signed sidecar over `stdio` JSONL/JSON-RPC with an app-specific Codex home. ChatGPT managed sign-in is primary; API-key sign-in is optional. Codex runs with a read-only sandbox and network disabled by default. Editing tools are exposed only through the bundled MCP adapter, which authenticates to the Rust dispatcher through a per-launch Unix socket and random capability token.

Ollama is contacted only on loopback. Models must pass a structured-tool capability test before editing is enabled. Calls are bounded by schema validation, cancellation, repeated-call detection, and a maximum tool-round count.

## Media path

The current functional preview uses the Tauri asset protocol for user-selected local media, automatically preferring a generated proxy, and the first export implementation uses FFmpeg. The native Swift/AVFoundation package remains the intended composition preview and normal-export path before release. FFmpeg and FFprobe currently handle inspection, thumbnails, waveform generation, proxies, scene/keyframe extraction, silence detection, and the validated fallback export. Analysis artifacts are recorded in the portable project document while their derived files remain reconstructable cache data. The release build will bundle an LGPL-compatible FFmpeg configuration without GPL or nonfree components. `whisper.cpp` transcription remains a later local-analysis milestone.

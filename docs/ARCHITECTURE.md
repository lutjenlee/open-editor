# Architecture

## Trust boundaries

React renders project snapshots and dispatches manual intents. Folder-backed mutations cross the Tauri boundary and are revalidated, revision-checked, persisted, and audited by the Rust command dispatcher. The in-memory TypeScript implementation supports the non-persistent UI demo and is contract-tested against the same envelope shape. A narrow C-compatible Swift boundary is reserved for AVFoundation composition playback. Neither a hosted nor local model receives direct write access to project files.

```text
React UI
   │ validated Tauri commands + events
Rust project engine ───────── Swift AVFoundation bridge
   │
    ├── bundled FFmpeg / FFprobe / whisper.cpp workers
    └── local MCP adapter ── command dispatcher

Future providers:
    Codex App Server ──┘
    Ollama loopback API ┘
```

Every manual or agent edit becomes an `EditorCommand` envelope containing a command, source, conversation and batch IDs, and expected project revision. The dispatcher validates it, applies it atomically, records forward and inverse patches, autosaves, and publishes a new immutable snapshot.

## Portable projects

`open-editor.project.json` is the versioned, human-readable source of truth. The hidden `.open-editor` directory holds history, migration backups, chats, and reconstructable caches. Writes use a synced temporary file and atomic rename. Older schemas migrate forward only after preserving the original document; newer unsupported schemas are rejected without modification. In-project media uses relative paths. External media stores a display path plus a macOS security-scoped bookmark; an invalid or missing file is surfaced for explicit relinking.

## Agent isolation

The future Codex App Server process will run as a pinned signed sidecar over `stdio` JSONL/JSON-RPC with an app-specific Codex home. ChatGPT managed sign-in is primary; API-key sign-in is optional. Codex will run with a read-only sandbox and network disabled by default. The provider-independent MCP adapter is implemented and bundled: it exposes immutable project snapshots, exact JSON Schemas for validated editor commands, atomic edit batches, and cancellable local media jobs. It authenticates to the Rust dispatcher through a mode-`0600` per-launch Unix socket and 256-bit random capability token. Only the active project ID is authorized; the MCP process never receives project-folder paths and has no direct write path.

Ollama is contacted only on loopback. Models must pass a structured-tool capability test before editing is enabled. Calls are bounded by schema validation, cancellation, repeated-call detection, and a maximum tool-round count.

## Media path

Swift builds the canonical timeline into `AVMutableComposition`, `AVVideoComposition`, Core Animation overlays, and `AVAudioMix` for both preview and normal H.264/AAC export. It handles trims, timing, speed, transforms, opacity, captions, images, fades, and cross-dissolves. FFmpeg and FFprobe handle inspection, thumbnails, waveform generation, proxies, scene/keyframe extraction, silence detection, audio conversion for Whisper, and fallback conversion. `whisper.cpp` produces timestamped local transcript artifacts. Long operations run through cancellable jobs. Project writers are serialized with a file lock; stale or same-revision conflicting saves are rejected.

Release media sidecars are reproducibly built from pinned source. FFmpeg is configured with `--disable-gpl --disable-nonfree --disable-network --disable-shared`; the resulting binaries report LGPL 2.1 or later and link only Apple system frameworks. Licenses and source/build instructions live in `THIRD_PARTY_NOTICES` and `scripts`.

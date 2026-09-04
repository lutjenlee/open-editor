# Open Editor

Open Editor is an open-source, local-first macOS video editor designed for natural-language editing. Bring your existing ChatGPT/Codex account or a local Ollama model; the editor turns approved model tool calls into the same validated, undoable commands used by its manual timeline controls.

> [!IMPORTANT]
> Open Editor is early-stage software. The current build supports folder projects, persistent macOS media bookmarks and relinking, real FFprobe inspection, generated thumbnails/waveforms/proxies, local scene and silence analysis, local media preview, command-based timeline edits, undo/redo, and a first H.264/AAC export path. It is not ready for production editing yet.

## Product principles

- Original media is never modified.
- Project files, generated assets, conversations, and edit history stay in a user-selected folder.
- Codex mode sends only explicitly approved derived context such as metadata, transcript excerpts, and selected keyframes.
- Ollama mode can remain fully local.
- Manual and AI edits share one command API and undo history.

## Technology

- Tauri 2, React, TypeScript, Zustand, and Rust
- AVFoundation through a small native Swift package
- FFmpeg and FFprobe for compatibility and media analysis
- `whisper.cpp` for local transcription
- Codex App Server over its stable `stdio` transport
- Ollama on loopback for local models

## Current development setup

Prerequisites:

- macOS 14 or newer
- Node.js 22 or newer
- Rust stable (`rustup` recommended)
- Full Xcode with its command-line tools selected
- FFmpeg/FFprobe available locally while sidecar packaging is under development

```bash
npm install
npm run dev
```

Run checks with:

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
swift test --package-path native/OpenEditorMedia
```

The provisional macOS bundle identifier is `com.lutjenlee.openeditor`. Changing it after signed releases begin can break app identity, Keychain access, and updates, so it should be finalized before the first signed release.

## Authentication

An OpenAI API key is **not required** for the intended default experience. The Codex provider will use Codex App Server's managed ChatGPT sign-in flow. API-key authentication may be offered as an optional alternative. Credentials remain managed by the App Server in an isolated Open Editor Codex home.

## Project format

Each project lives in a folder selected by the user:

```text
My Project/
├── open-editor.project.json
└── .open-editor/
    ├── history.jsonl
    ├── chats/
    ├── cache/
    ├── proxies/
    ├── thumbnails/
    ├── waveforms/
    └── transcripts/
```

See [Architecture](docs/ARCHITECTURE.md), [Contributing](CONTRIBUTING.md), and [Security](SECURITY.md).

## Status

The current development focus is the provider-independent local editor. Codex and Ollama are deliberately not connected yet. A durable Rust dispatcher accepts revision-checked command envelopes and rejects provider attempts to expand the user-approved media scope. A bundled MCP sidecar reaches that dispatcher only through a mode-`0600` per-launch Unix socket, a random capability token, and an app-authorized project ID. It exposes read-only project snapshots and validated editor commands; it has no direct project-file access.

Current local workflow:

1. Create or open a folder-backed project.
2. Import supported media and generate local inspection artifacts.
3. Double-click media to add it to the appropriate track.
4. Move, split, delete, undo, redo, save, reopen, and preview selected media.
5. Export the video track to a local H.264/AAC MP4.

Still in progress before the provider milestone: native AVFoundation composition playback, complete multitrack audio/overlay/caption rendering, cancellable background-job progress, offline transcription, and release packaging for the remaining media sidecars.

## License

Licensed under the [Apache License 2.0](LICENSE).

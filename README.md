# Open Editor

Open Editor is an open-source, local-first macOS video editor designed for natural-language editing. Bring your existing ChatGPT/Codex account or a local Ollama model; the editor turns approved model tool calls into the same validated, undoable commands used by its manual timeline controls.

> [!IMPORTANT]
> Open Editor is early-stage software. The current build supports folder projects, persistent macOS media bookmarks and relinking, bundled media tools, generated thumbnails/waveforms/proxies, local scene/silence/transcript analysis, AVFoundation composition playback and export, command-based timeline edits, alternatives, and undo/redo. It is not ready for production editing yet.

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
- CMake (only when rebuilding the pinned whisper.cpp and FFmpeg sidecars)

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
    ├── backups/
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

1. Create a folder-backed project, or open a folder that already contains `open-editor.project.json`.
2. Import individual files, an entire media folder (including nested folders), or drag files into the window.
3. Select media to preview it, then choose **Add to timeline** (double-click remains available as a shortcut).
4. Move, trim, split, duplicate, transform, mix, caption, transition, undo, redo, and create alternative sequences.
5. Preview the composition and export an H.264/AAC MP4 through AVFoundation.

Proxy creation, analysis, transcription, and export run as observable, cancellable background jobs. FFmpeg 9.0.1, FFprobe, whisper.cpp 1.9.1, and the MCP adapter are pinned build-time sidecars; the FFmpeg configuration disables GPL and nonfree components. The provider milestone begins only after this local command boundary is stable. See [Codex readiness](docs/CODEX_READINESS.md).

## License

Licensed under the [Apache License 2.0](LICENSE).

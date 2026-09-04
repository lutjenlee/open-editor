# Codex readiness boundary

Open Editor deliberately completes the local editor before connecting Codex App Server. This keeps model integration thin: Codex will orchestrate already-tested capabilities rather than becoming part of the media engine.

## Ready now

- Folder-backed, versioned projects with atomic saves, migration backups, revision checks, history, relinking, and reconstructable caches.
- Stable UUID-based sequence, track, clip, caption, transition, media, conversation, and analysis records using rational time.
- One validated command dispatcher shared by manual controls and future providers.
- Atomic provider batches: either every command is saved as one audit/undo group or none are.
- Alternative-sequence duplication with fresh mutable entity IDs.
- Native AVFoundation composition preview and export using the same project snapshot.
- Bundled, pinned FFmpeg/FFprobe and whisper.cpp workers for local inspection, proxies, analysis, and transcription.
- A bundled MCP adapter exposing project snapshots, exact command schemas, atomic batches, and cancellable media jobs.
- A mode-`0600` Unix socket, random per-launch capability token, and active-project authorization between MCP and Rust. Models receive no project-folder path and cannot add or remove approved media.

## The next milestone: Codex App Server only

1. Pin and bundle a tested Codex App Server binary and generated protocol schemas.
2. Launch it with an isolated Open Editor Codex home over stable `stdio` JSONL/JSON-RPC.
3. Implement `initialize`/`initialized`, managed ChatGPT sign-in, optional API-key sign-in, account state, and model listing.
4. Implement project conversation create/resume/archive plus streamed turn/item events, interruption, approvals, errors, and rate limits.
5. Register the bundled Open Editor MCP adapter with the socket path and capability token for the active project.
6. Add the first-use hosted-context disclosure and per-turn “Context shared” inspector. Send only selected metadata, transcript excerpts, and compressed keyframes—never source media automatically.
7. Replay recorded App Server fixtures and complete an end-to-end agent edit against a disposable project before enabling the UI composer.

Ollama remains a later, separate provider. Signing, notarization, updater publication, and universal release smoke tests belong to release hardening and require release credentials/hardware; they are not prerequisites for beginning the Codex provider milestone.

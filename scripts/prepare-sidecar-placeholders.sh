#!/usr/bin/env bash
set -euo pipefail

export PATH="${CARGO_HOME:-${HOME}/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:${PATH}"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
target="$(rustc -vV | awk '/^host:/ { print $2 }')"
mkdir -p "$repo_root/src-tauri/binaries"
for program in open-editor-mcp ffmpeg ffprobe whisper-cli; do
  path="$repo_root/src-tauri/binaries/$program-$target"
  [[ -e "$path" ]] || : > "$path"
done

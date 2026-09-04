#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
host_target="$(rustc -vV | awk '/^host:/ { print $2 }')"
target="${OPEN_EDITOR_TARGET:-$host_target}"
output_dir="$repo_root/src-tauri/binaries"
mkdir -p "$output_dir"
# Tauri validates externalBin paths from this package's build script, including
# while Cargo is compiling the sidecar itself. Stage the expected path first.
: > "$output_dir/open-editor-mcp-$target"

if [[ "$target" == "universal-apple-darwin" ]]; then
  for architecture in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --manifest-path "$repo_root/src-tauri/Cargo.toml" --release --bin open-editor-mcp --target "$architecture"
  done
  xcrun lipo -create \
    "$repo_root/src-tauri/target/aarch64-apple-darwin/release/open-editor-mcp" \
    "$repo_root/src-tauri/target/x86_64-apple-darwin/release/open-editor-mcp" \
    -output "$output_dir/open-editor-mcp-universal-apple-darwin"
else
  cargo build --manifest-path "$repo_root/src-tauri/Cargo.toml" --release --bin open-editor-mcp --target "$target"
  cp "$repo_root/src-tauri/target/$target/release/open-editor-mcp" "$output_dir/open-editor-mcp-$target"
fi

chmod 755 "$output_dir/open-editor-mcp-$target"

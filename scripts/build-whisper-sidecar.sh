#!/usr/bin/env bash
set -euo pipefail
export PATH="${CARGO_HOME:-${HOME}/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:${PATH}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
version="v1.9.1"
host_target="$(rustc -vV | awk '/^host:/ { print $2 }')"
target="${OPEN_EDITOR_TARGET:-$host_target}"
output_dir="$repo_root/src-tauri/binaries"
source_root="$repo_root/.build/whisper.cpp-$version"
archive="$repo_root/.build/whisper.cpp-$version.tar.gz"
mkdir -p "$output_dir" "$repo_root/.build" "$repo_root/THIRD_PARTY_NOTICES"

if [[ "${OPEN_EDITOR_REBUILD_SIDECARS:-0}" != "1" \
  && -x "$output_dir/whisper-cli-$target" ]] \
  && "$output_dir/whisper-cli-$target" --version 2>/dev/null | grep -q "whisper.cpp version: ${version#v}"; then
  exit 0
fi

if [[ ! -f "$source_root/CMakeLists.txt" ]]; then
  curl --fail --location --silent --show-error \
    "https://github.com/ggml-org/whisper.cpp/archive/refs/tags/$version.tar.gz" \
    --output "$archive"
  tar -xzf "$archive" -C "$repo_root/.build"
  extracted="$repo_root/.build/whisper.cpp-${version#v}"
  mv "$extracted" "$source_root"
fi

cp "$source_root/LICENSE" "$repo_root/THIRD_PARTY_NOTICES/whisper.cpp-LICENSE"

build_architecture() {
  architecture="$1"
  build_dir="$source_root/build-$architecture"
  cmake -S "$source_root" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=14.0 \
    -DCMAKE_OSX_ARCHITECTURES="$architecture" \
    -DBUILD_SHARED_LIBS=OFF \
    -DWHISPER_BUILD_TESTS=OFF \
    -DWHISPER_BUILD_SERVER=OFF \
    -DWHISPER_BUILD_EXAMPLES=ON >&2
  cmake --build "$build_dir" --config Release --target whisper-cli -j 4 >&2
  printf '%s' "$build_dir/bin/whisper-cli"
}

if [[ "$target" == "universal-apple-darwin" ]]; then
  arm_binary="$(build_architecture arm64)"
  intel_binary="$(build_architecture x86_64)"
  xcrun lipo -create "$arm_binary" "$intel_binary" -output "$output_dir/whisper-cli-$target"
else
  case "$target" in
    aarch64-apple-darwin) architecture="arm64" ;;
    x86_64-apple-darwin) architecture="x86_64" ;;
    *) echo "Unsupported whisper.cpp target: $target" >&2; exit 2 ;;
  esac
  binary="$(build_architecture "$architecture")"
  cp "$binary" "$output_dir/whisper-cli-$target"
fi

chmod 755 "$output_dir/whisper-cli-$target"

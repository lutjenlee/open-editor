#!/usr/bin/env bash
set -euo pipefail

export PATH="${CARGO_HOME:-${HOME}/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:${PATH}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
version="9.0.1"
archive_sha256="cf38e0e28c7e5605942c4a77755349b0145804a397af37eb1fb4c77cb237f635"
host_target="$(rustc -vV | awk '/^host:/ { print $2 }')"
target="${OPEN_EDITOR_TARGET:-$host_target}"
output_dir="$repo_root/src-tauri/binaries"
source_root="${TMPDIR:-/private/tmp}/open-editor-ffmpeg-$version"
archive="$repo_root/.build/ffmpeg-$version.tar.xz"

mkdir -p "$output_dir" "$repo_root/.build" "$repo_root/THIRD_PARTY_NOTICES"

if [[ "${OPEN_EDITOR_REBUILD_SIDECARS:-0}" != "1" \
  && -x "$output_dir/ffmpeg-$target" \
  && -x "$output_dir/ffprobe-$target" ]] \
  && "$output_dir/ffmpeg-$target" -version 2>/dev/null | head -n 1 | grep -q "ffmpeg version $version" \
  && "$output_dir/ffmpeg-$target" -L 2>/dev/null | grep -q "GNU Lesser General Public License"; then
  exit 0
fi

if [[ ! -f "$archive" ]]; then
  curl --fail --location --silent --show-error \
    "https://ffmpeg.org/releases/ffmpeg-$version.tar.xz" \
    --output "$archive.download"
  actual="$(shasum -a 256 "$archive.download" | awk '{ print $1 }')"
  [[ "$actual" == "$archive_sha256" ]] || { echo "FFmpeg archive checksum mismatch" >&2; exit 1; }
  mv "$archive.download" "$archive"
fi

actual="$(shasum -a 256 "$archive" | awk '{ print $1 }')"
[[ "$actual" == "$archive_sha256" ]] || { echo "FFmpeg archive checksum mismatch" >&2; exit 1; }

if [[ ! -f "$source_root/configure" ]]; then
  extraction_root="${TMPDIR:-/private/tmp}/open-editor-ffmpeg-extract-$version"
  mkdir -p "$extraction_root"
  tar -xJf "$archive" -C "$extraction_root"
  mv "$extraction_root/ffmpeg-$version" "$source_root"
fi

cp "$source_root/COPYING.LGPLv2.1" "$repo_root/THIRD_PARTY_NOTICES/FFmpeg-LGPL-2.1.txt"

build_architecture() {
  architecture="$1"
  build_dir="$source_root/open-editor-build-$architecture"
  install_dir="$source_root/open-editor-install-$architecture"
  mkdir -p "$build_dir" "$install_dir"
  (
    cd "$build_dir"
    "$source_root/configure" \
      --prefix="$install_dir" \
      --cc="xcrun clang -arch $architecture" \
      --arch="$architecture" \
      --target-os=darwin \
      --extra-cflags="-mmacosx-version-min=14.0" \
      --extra-ldflags="-mmacosx-version-min=14.0" \
      --disable-gpl \
      --disable-nonfree \
      --disable-doc \
      --disable-debug \
      --disable-network \
      --disable-autodetect \
      --disable-shared \
      --enable-static \
      --enable-audiotoolbox \
      --enable-videotoolbox
    make -j 4 ffmpeg ffprobe
  ) >&2
  printf '%s' "$build_dir"
}

if [[ "$target" == "universal-apple-darwin" ]]; then
  arm_build="$(build_architecture arm64)"
  intel_build="$(build_architecture x86_64)"
  xcrun lipo -create "$arm_build/ffmpeg" "$intel_build/ffmpeg" -output "$output_dir/ffmpeg-$target.download"
  xcrun lipo -create "$arm_build/ffprobe" "$intel_build/ffprobe" -output "$output_dir/ffprobe-$target.download"
else
  case "$target" in
    aarch64-apple-darwin) architecture="arm64" ;;
    x86_64-apple-darwin) architecture="x86_64" ;;
    *) echo "Unsupported FFmpeg target: $target" >&2; exit 2 ;;
  esac
  build_dir="$(build_architecture "$architecture")"
  cp "$build_dir/ffmpeg" "$output_dir/ffmpeg-$target.download"
  cp "$build_dir/ffprobe" "$output_dir/ffprobe-$target.download"
fi

for program in ffmpeg ffprobe; do
  mv "$output_dir/$program-$target.download" "$output_dir/$program-$target"
  chmod 755 "$output_dir/$program-$target"
done

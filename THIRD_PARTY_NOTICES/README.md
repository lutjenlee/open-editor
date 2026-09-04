# Third-party notices

Open Editor bundles command-line builds of the following projects:

- **FFmpeg 9.0.1**, configured without GPL or nonfree components and used under LGPL 2.1 or later. The unmodified source archive is available from <https://ffmpeg.org/releases/ffmpeg-9.0.1.tar.xz>. The exact reproducible build configuration is in `scripts/build-ffmpeg-sidecars.sh`. The license text is included as `FFmpeg-LGPL-2.1.txt` when the sidecars are built.
- **whisper.cpp 1.9.1**, used under the MIT License. The license text is included as `whisper.cpp-LICENSE`.

Open Editor invokes these tools as separate local processes. No GPL or nonfree FFmpeg options are enabled by the provided release build.

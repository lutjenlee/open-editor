use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use thiserror::Error;
use uuid::Uuid;

use crate::project::RationalTime;

#[derive(Debug, Error, Serialize)]
pub enum MediaError {
    #[error("Media tool not found: {0}")]
    ToolMissing(String),
    #[error("Media operation failed: {0}")]
    Failed(String),
    #[error("Unsupported media file: {0}")]
    Unsupported(String),
    #[error("Could not read media metadata: {0}")]
    Decode(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInspection {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub duration: RationalTime,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codec: Option<String>,
    pub has_audio: bool,
    pub thumbnail_path: Option<String>,
    pub waveform_path: Option<String>,
}

fn tool(name: &str) -> Result<PathBuf, MediaError> {
    for path in [
        format!("/opt/homebrew/bin/{name}"),
        format!("/usr/local/bin/{name}"),
    ] {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let output = Command::new("/usr/bin/which")
        .arg(name)
        .output()
        .map_err(|_| MediaError::ToolMissing(name.into()))?;
    if output.status.success() {
        return Ok(PathBuf::from(
            String::from_utf8_lossy(&output.stdout).trim(),
        ));
    }
    Err(MediaError::ToolMissing(format!(
        "{name}. Install an LGPL-compatible FFmpeg build."
    )))
}

fn seconds(value: f64) -> RationalTime {
    RationalTime {
        value: (value * 600.0).round() as i64,
        timescale: 600,
    }
}

pub fn inspect(path: &Path, project_folder: &Path) -> Result<MediaInspection, MediaError> {
    if !path.is_file() {
        return Err(MediaError::Unsupported(path.display().to_string()));
    }
    let output = Command::new(tool("ffprobe")?)
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| MediaError::Failed(e.to_string()))?;
    if !output.status.success() {
        return Err(MediaError::Failed(
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ));
    }
    let data: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| MediaError::Decode(e.to_string()))?;
    let streams = data["streams"]
        .as_array()
        .ok_or_else(|| MediaError::Decode("missing streams".into()))?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video");
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let image_ext = matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "heic" | "webp");
    let kind = if video.is_some() && image_ext {
        "image"
    } else if video.is_some() {
        "video"
    } else if audio.is_some() {
        "audio"
    } else {
        return Err(MediaError::Unsupported(path.display().to_string()));
    };
    let duration = data["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(if kind == "image" { 5.0 } else { 0.0 });
    let cache = project_folder.join(".open-editor");
    let id = Uuid::new_v4();
    let mut thumbnail = None;
    let mut waveform = None;
    if video.is_some() {
        let output_path = cache.join("thumbnails").join(format!("{id}.jpg"));
        let result = Command::new(tool("ffmpeg")?)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-ss",
                "0.1",
                "-i",
            ])
            .arg(path)
            .args(["-frames:v", "1", "-vf", "scale=480:-2"])
            .arg(&output_path)
            .status();
        if result.is_ok_and(|status| status.success()) {
            thumbnail = Some(output_path.display().to_string());
        }
    } else if audio.is_some() {
        let output_path = cache.join("waveforms").join(format!("{id}.png"));
        let result = Command::new(tool("ffmpeg")?)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(path)
            .args([
                "-filter_complex",
                "showwavespic=s=1200x160:colors=7ac5a5",
                "-frames:v",
                "1",
            ])
            .arg(&output_path)
            .status();
        if result.is_ok_and(|status| status.success()) {
            waveform = Some(output_path.display().to_string());
        }
    }
    let primary = video.or(audio);
    Ok(MediaInspection {
        path: path.display().to_string(),
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Media")
            .into(),
        kind: kind.into(),
        duration: seconds(duration),
        width: video
            .and_then(|stream| stream["width"].as_u64())
            .map(|value| value as u32),
        height: video
            .and_then(|stream| stream["height"].as_u64())
            .map(|value| value as u32),
        codec: primary
            .and_then(|stream| stream["codec_name"].as_str())
            .map(str::to_string),
        has_audio: audio.is_some(),
        thumbnail_path: thumbnail,
        waveform_path: waveform,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportClip {
    pub source_path: String,
    pub source_in: RationalTime,
    pub source_out: RationalTime,
    pub playback_rate: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalTime,
    pub clips: Vec<ExportClip>,
}

pub fn export(request: ExportRequest) -> Result<String, MediaError> {
    if request.clips.is_empty() {
        return Err(MediaError::Failed(
            "Add at least one video clip before exporting.".into(),
        ));
    }
    let fps = request.frame_rate.value as f64 / request.frame_rate.timescale as f64;
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
    ];
    let mut total = 0.0;
    for clip in &request.clips {
        let start = clip.source_in.value as f64 / clip.source_in.timescale as f64;
        let end = clip.source_out.value as f64 / clip.source_out.timescale as f64;
        total += (end - start) / clip.playback_rate;
        args.extend([
            "-ss".into(),
            format!("{start:.6}"),
            "-to".into(),
            format!("{end:.6}"),
            "-i".into(),
            clip.source_path.clone(),
        ]);
    }
    args.extend([
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "anullsrc=channel_layout=stereo:sample_rate=48000".into(),
    ]);
    let mut filter = String::new();
    let mut inputs = String::new();
    for (index, clip) in request.clips.iter().enumerate() {
        filter.push_str(&format!("[{index}:v]scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps},setpts=PTS/{:.6}[v{index}];", request.width, request.height, request.width, request.height, clip.playback_rate));
        inputs.push_str(&format!("[v{index}]"));
    }
    filter.push_str(&format!(
        "{inputs}concat=n={}:v=1:a=0[outv]",
        request.clips.len()
    ));
    args.extend([
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[outv]".into(),
        "-map".into(),
        format!("{}:a", request.clips.len()),
        "-t".into(),
        format!("{total:.6}"),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-shortest".into(),
        request.output_path.clone(),
    ]);
    let output = Command::new(tool("ffmpeg")?)
        .args(args)
        .output()
        .map_err(|e| MediaError::Failed(e.to_string()))?;
    if !output.status.success() {
        return Err(MediaError::Failed(
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ));
    }
    Ok(request.output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_and_exports_fixture_when_ffmpeg_is_available() {
        let Ok(ffmpeg) = tool("ffmpeg") else { return };
        let root = tempfile::tempdir().unwrap();
        crate::project::initialize_layout(root.path()).unwrap();
        let source = root.path().join("fixture.mp4");
        let generated = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x180:d=1",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=48000:cl=stereo",
                "-shortest",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(generated.success());
        let inspection = inspect(&source, root.path()).unwrap();
        assert_eq!(inspection.kind, "video");
        assert_eq!(inspection.width, Some(320));
        let output = root.path().join("export.mp4");
        export(ExportRequest {
            output_path: output.display().to_string(),
            width: 180,
            height: 320,
            frame_rate: RationalTime {
                value: 30,
                timescale: 1,
            },
            clips: vec![ExportClip {
                source_path: source.display().to_string(),
                source_in: seconds(0.0),
                source_out: seconds(0.8),
                playback_rate: 1.0,
            }],
        })
        .unwrap();
        assert!(output.metadata().unwrap().len() > 0);
    }
}

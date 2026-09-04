use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use thiserror::Error;
use uuid::Uuid;

use crate::jobs::JobContext;
use crate::project::RationalTime;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceRange {
    pub start: RationalTime,
    pub end: RationalTime,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAnalysis {
    pub scene_times: Vec<RationalTime>,
    pub silence_ranges: Vec<SilenceRange>,
    pub keyframe_paths: Vec<String>,
}

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
    #[error("Media operation was cancelled")]
    Cancelled,
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
    pub bookmark: Option<String>,
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
        bookmark: crate::native::create_security_bookmark(path),
        thumbnail_path: thumbnail,
        waveform_path: waveform,
    })
}

fn run_ffmpeg(
    args: &[String],
    job: Option<&JobContext>,
) -> Result<std::process::Output, MediaError> {
    if job.is_some_and(JobContext::is_cancelled) {
        return Err(MediaError::Cancelled);
    }
    let child = Command::new(tool("ffmpeg")?)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| MediaError::Failed(error.to_string()))?;
    if let Some(job) = job {
        job.register_process(Some(child.id()));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| MediaError::Failed(error.to_string()))?;
    if let Some(job) = job {
        job.register_process(None);
        if job.is_cancelled() {
            return Err(MediaError::Cancelled);
        }
    }
    Ok(output)
}

pub fn create_proxy(
    source: &Path,
    project_folder: &Path,
    asset_id: Uuid,
    job: Option<&JobContext>,
) -> Result<String, MediaError> {
    let output_path = project_folder
        .join(".open-editor/proxies")
        .join(format!("{asset_id}.mp4"));
    let base = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        source.display().to_string(),
        "-vf".into(),
        "scale='min(1280,iw)':-2".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "160k".into(),
        "-movflags".into(),
        "+faststart".into(),
    ];
    let mut hardware = base.clone();
    hardware.extend([
        "-c:v".into(),
        "h264_videotoolbox".into(),
        "-b:v".into(),
        "5M".into(),
        output_path.display().to_string(),
    ]);
    if let Some(job) = job {
        job.running("Creating playback proxy", 0.1);
    }
    let first = match run_ffmpeg(&hardware, job) {
        Ok(output) => output,
        Err(error) => {
            let _ = std::fs::remove_file(&output_path);
            return Err(error);
        }
    };
    if !first.status.success() {
        let mut portable = base;
        portable.extend([
            "-c:v".into(),
            "mpeg4".into(),
            "-q:v".into(),
            "4".into(),
            output_path.display().to_string(),
        ]);
        if let Some(job) = job {
            job.running("Using compatible proxy encoder", 0.45);
        }
        let second = match run_ffmpeg(&portable, job) {
            Ok(output) => output,
            Err(error) => {
                let _ = std::fs::remove_file(&output_path);
                return Err(error);
            }
        };
        if !second.status.success() {
            return Err(MediaError::Failed(
                String::from_utf8_lossy(&second.stderr).trim().into(),
            ));
        }
    }
    Ok(output_path.display().to_string())
}

fn parse_number_after(line: &str, marker: &str) -> Option<f64> {
    line.split(marker)
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

pub fn analyze(
    source: &Path,
    project_folder: &Path,
    asset_id: Uuid,
    job: Option<&JobContext>,
) -> Result<LocalAnalysis, MediaError> {
    let analysis_dir = project_folder
        .join(".open-editor/cache/analysis")
        .join(asset_id.to_string());
    std::fs::create_dir_all(&analysis_dir)
        .map_err(|error| MediaError::Failed(error.to_string()))?;

    let scene_pattern = analysis_dir.join("scene-%04d.jpg");
    let scene_args = vec![
        "-hide_banner".into(),
        "-y".into(),
        "-i".into(),
        source.display().to_string(),
        "-vf".into(),
        "select=gt(scene\\,0.32),showinfo,scale=480:-2".into(),
        "-fps_mode".into(),
        "vfr".into(),
        scene_pattern.display().to_string(),
    ];
    if let Some(job) = job {
        job.running("Detecting scenes and extracting keyframes", 0.1);
    }
    let scene_output = run_ffmpeg(&scene_args, job)?;
    let scene_log = String::from_utf8_lossy(&scene_output.stderr);
    let scene_times = scene_log
        .lines()
        .filter_map(|line| parse_number_after(line, "pts_time:"))
        .map(seconds)
        .collect::<Vec<_>>();

    let silence_args = vec![
        "-hide_banner".into(),
        "-i".into(),
        source.display().to_string(),
        "-af".into(),
        "silencedetect=noise=-35dB:d=0.35".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ];
    if let Some(job) = job {
        job.running("Detecting silence", 0.65);
    }
    let silence_output = run_ffmpeg(&silence_args, job)?;
    let mut open_start = None;
    let mut silence_ranges = Vec::new();
    for line in String::from_utf8_lossy(&silence_output.stderr).lines() {
        if let Some(value) = parse_number_after(line, "silence_start:") {
            open_start = Some(value);
        }
        if let (Some(start), Some(end)) = (open_start, parse_number_after(line, "silence_end:")) {
            silence_ranges.push(SilenceRange {
                start: seconds(start),
                end: seconds(end),
            });
            open_start = None;
        }
    }
    let mut keyframe_paths = std::fs::read_dir(&analysis_dir)
        .map_err(|error| MediaError::Failed(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "jpg"))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    keyframe_paths.sort();
    Ok(LocalAnalysis {
        scene_times,
        silence_ranges,
        keyframe_paths,
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

pub fn export(request: ExportRequest, job: Option<&JobContext>) -> Result<String, MediaError> {
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
    if let Some(job) = job {
        job.running("Rendering video", 0.05);
    }
    let output = match run_ffmpeg(&args, job) {
        Ok(output) => output,
        Err(error) => {
            let _ = std::fs::remove_file(&request.output_path);
            return Err(error);
        }
    };
    if !output.status.success() {
        let _ = std::fs::remove_file(&request.output_path);
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
        let asset_id = Uuid::new_v4();
        let proxy = create_proxy(&source, root.path(), asset_id, None).unwrap();
        assert!(Path::new(&proxy).metadata().unwrap().len() > 0);
        let analysis = analyze(&source, root.path(), asset_id, None).unwrap();
        assert!(analysis
            .scene_times
            .iter()
            .all(|time| time.timescale == 600));
        let output = root.path().join("export.mp4");
        export(
            ExportRequest {
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
            },
            None,
        )
        .unwrap();
        assert!(output.metadata().unwrap().len() > 0);
    }
}

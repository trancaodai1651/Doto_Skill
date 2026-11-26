//! ffprobe → MediaInfo, và tạo thumbnail.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::ffmpeg::{command, run_with_timeout};

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub has_video: bool,
    pub has_audio: bool,
}

pub fn parse_ffprobe_json(json: &str) -> anyhow::Result<MediaInfo> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let mut info = MediaInfo::default();
    if let Some(streams) = v["streams"].as_array() {
        for s in streams {
            match s["codec_type"].as_str() {
                Some("video") if !info.has_video && s["disposition"]["attached_pic"].as_i64() != Some(1) => {
                    info.has_video = true;
                    info.video_codec = s["codec_name"].as_str().map(String::from);
                    info.width = s["width"].as_i64();
                    info.height = s["height"].as_i64();
                    info.fps = s["avg_frame_rate"].as_str().and_then(parse_rational).filter(|f| *f > 0.0 && *f < 1000.0);
                }
                Some("audio") if !info.has_audio => {
                    info.has_audio = true;
                    info.audio_codec = s["codec_name"].as_str().map(String::from);
                }
                _ => {}
            }
        }
    }
    info.duration_ms = v["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).map(|d| (d * 1000.0).round() as i64);
    info.container = v["format"]["format_name"].as_str().map(|s| s.split(',').next().unwrap_or(s).to_string());
    anyhow::ensure!(info.has_video, "file không có luồng video hợp lệ");
    Ok(info)
}

fn parse_rational(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
    (d != 0.0).then(|| n / d)
}

pub async fn probe(ffprobe: &Path, file: &Path) -> anyhow::Result<MediaInfo> {
    let path = file.to_string_lossy().into_owned();
    let out = run_with_timeout(ffprobe, &["-v", "error", "-print_format", "json", "-show_format", "-show_streams", &path], Duration::from_secs(30)).await?;
    parse_ffprobe_json(&out)
}

/// Thumbnail JPEG rộng 480px tại ~10% thời lượng (tối đa giây 5). Ghi vào file tạm rồi rename (atomic).
pub async fn thumbnail(ffmpeg: &Path, file: &Path, duration_ms: Option<i64>, out: &Path) -> anyhow::Result<()> {
    let at = duration_ms.map(|d| (d as f64 / 1000.0 * 0.1).min(5.0)).unwrap_or(0.0);
    let tmp = out.with_extension("part.jpg");
    let mut cmd = command(ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-ss", &format!("{at:.2}"), "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", "scale=480:-2", "-q:v", "4"])
        .arg(&tmp);
    let res = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await;
    let ok = matches!(&res, Ok(Ok(o)) if o.status.success()) && tmp.is_file();
    if !ok {
        let _ = tokio::fs::remove_file(&tmp).await;
        anyhow::bail!("không tạo được thumbnail");
    }
    tokio::fs::rename(&tmp, out).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_ffprobe_output() {
        let j = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},
                    {"codec_type":"audio","codec_name":"aac"}],
                    "format":{"duration":"12.345","format_name":"mov,mp4,m4a,3gp,3g2,mj2"}}"#;
        let i = parse_ffprobe_json(j).unwrap();
        assert_eq!(i.duration_ms, Some(12345));
        assert_eq!(i.width, Some(1920));
        assert_eq!(i.video_codec.as_deref(), Some("h264"));
        assert_eq!(i.audio_codec.as_deref(), Some("aac"));
        assert_eq!(i.container.as_deref(), Some("mov"));
        assert!((i.fps.unwrap() - 29.97).abs() < 0.01);
    }

    #[test]
    fn rejects_audio_only_or_garbage() {
        assert!(parse_ffprobe_json(r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"}],"format":{}}"#).is_err());
        assert!(parse_ffprobe_json("not json").is_err());
    }

    #[test]
    fn cover_art_is_not_video() {
        let j = r#"{"streams":[{"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}],"format":{}}"#;
        assert!(parse_ffprobe_json(j).is_err());
    }
}

//! Probe encoder H.264 thực tế: không dựa vào tên card mà encode thử 1 giây khung hình tổng hợp.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ffmpeg::{command, run_with_timeout};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EncoderInfo {
    pub name: String,
    pub kind: String,
    pub vendor: String,
    pub available: bool,
    pub working: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EncoderBenchmark {
    pub name: String,
    pub kind: String,
    pub vendor: String,
    pub frames: u32,
    pub elapsed_ms: u64,
    pub fps: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EncoderProbe {
    pub encoders: Vec<EncoderInfo>,
    pub probed_at: Option<String>,
    pub ffmpeg_version: Option<String>,
}

impl EncoderProbe {
    /// Encoder phần cứng tốt nhất đã kiểm chứng, theo thứ tự ưu tiên.
    pub fn best_hardware(&self) -> Option<&str> {
        HW_PRIORITY.iter().find(|n| self.encoders.iter().any(|e| &e.name == *n && e.working)).copied()
    }
    pub fn is_working(&self, name: &str) -> bool {
        name == "libx264" || self.encoders.iter().any(|e| e.name == name && e.working)
    }
}

const HW_PRIORITY: &[&str] = &["h264_nvenc", "h264_videotoolbox", "h264_qsv", "h264_amf", "h264_mf"];

pub fn candidates() -> Vec<(&'static str, &'static str, &'static str)> {
    // (tên encoder, kind, vendor) — chỉ thử những gì có ý nghĩa trên OS hiện tại
    let mut v = vec![("libx264", "software", "x264")];
    if cfg!(target_os = "macos") {
        v.push(("h264_videotoolbox", "hardware", "apple"));
    } else {
        v.push(("h264_nvenc", "hardware", "nvidia"));
        v.push(("h264_qsv", "hardware", "intel"));
        v.push(("h264_amf", "hardware", "amd"));
        if cfg!(windows) {
            v.push(("h264_mf", "hardware", "microsoft"));
        }
    }
    v
}

pub fn parse_encoder_list(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let mut it = l.split_whitespace();
            let flags = it.next()?;
            let name = it.next()?;
            (flags.len() == 6 && flags.starts_with('V')).then(|| name.to_string())
        })
        .collect()
}

/// Encode thử 1 giây 256x256 @30fps vào null muxer. Thành công ⇒ encoder dùng được thật.
pub async fn test_encoder(ffmpeg: &Path, name: &str) -> Result<(), String> {
    let mut cmd = command(ffmpeg);
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=256x256:rate=30:duration=1",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        name,
        "-frames:v",
        "30",
        "-f",
        "null",
        "-",
    ]);
    match tokio::time::timeout(Duration::from_secs(20), cmd.output()).await {
        Err(_) => Err("quá thời gian encode thử".into()),
        Ok(Err(e)) => Err(e.to_string()),
        Ok(Ok(o)) if o.status.success() => Ok(()),
        Ok(Ok(o)) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("lỗi không rõ").chars().take(300).collect()),
    }
}

pub async fn probe_all(ffmpeg: &Path, version: Option<String>) -> anyhow::Result<EncoderProbe> {
    let list = run_with_timeout(ffmpeg, &["-hide_banner", "-encoders"], Duration::from_secs(15)).await?;
    let names = parse_encoder_list(&list);
    let mut encoders = vec![];
    for (name, kind, vendor) in candidates() {
        let available = names.iter().any(|n| n == name);
        let (working, error) = if available {
            match test_encoder(ffmpeg, name).await {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            }
        } else {
            (false, Some("bản FFmpeg này không biên dịch kèm encoder".into()))
        };
        encoders.push(EncoderInfo { name: name.into(), kind: kind.into(), vendor: vendor.into(), available, working, error });
    }
    Ok(EncoderProbe { encoders, probed_at: Some(doto_core::now_rfc3339()), ffmpeg_version: version })
}

/// Benchmark ngắn (60 khung hình 1280x720 @30fps → null muxer) trên các encoder đã hoạt động.
pub async fn benchmark_working(ffmpeg: &Path, probe: &EncoderProbe, threads: usize) -> Vec<EncoderBenchmark> {
    const FRAMES: u32 = 60;
    let mut list: Vec<(String, String, String)> =
        probe.encoders.iter().filter(|e| e.working).map(|e| (e.name.clone(), e.kind.clone(), e.vendor.clone())).collect();
    if list.is_empty() {
        list.push(("libx264".into(), "software".into(), "x264".into()));
    }
    let mut out = vec![];
    for (name, kind, vendor) in list {
        let mut cmd = command(ffmpeg);
        cmd.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30:duration=2",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            &name,
        ]);
        if name == "libx264" {
            cmd.args(["-preset", "veryfast", "-threads", &threads.max(1).to_string()]);
        }
        cmd.args(["-frames:v", &FRAMES.to_string(), "-f", "null", "-"]);
        let t0 = std::time::Instant::now();
        if let Ok(Ok(res)) = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await {
            if res.status.success() {
                let elapsed_ms = t0.elapsed().as_millis().max(1) as u64;
                let fps = ((FRAMES as f64 * 1000.0 / elapsed_ms as f64) * 10.0).round() / 10.0;
                out.push(EncoderBenchmark { name, kind, vendor, frames: FRAMES, elapsed_ms, fps });
            }
        }
    }
    out.sort_by(|a, b| b.fps.partial_cmp(&a.fps).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_table() {
        let out = " V....D libx264              libx264 H.264 / AVC\n V....D h264_nvenc           NVIDIA NVENC\n A....D aac                  AAC\n ------\n";
        let n = parse_encoder_list(out);
        assert_eq!(n, vec!["libx264", "h264_nvenc"]);
    }

    #[test]
    fn best_hardware_respects_priority_and_working() {
        let mk = |n: &str, w: bool| EncoderInfo { name: n.into(), kind: "hardware".into(), vendor: "x".into(), available: true, working: w, error: None };
        let p = EncoderProbe { encoders: vec![mk("h264_qsv", true), mk("h264_nvenc", false)], ..Default::default() };
        assert_eq!(p.best_hardware(), Some("h264_qsv"));
        assert!(p.is_working("libx264"));
        assert!(!p.is_working("h264_nvenc"));
    }
}

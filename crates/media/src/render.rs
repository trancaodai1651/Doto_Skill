//! Pipeline render thật: trim + ghép nhiều đoạn → MP4 H.264/AAC.
//! Tiến độ/ETA đọc từ `-progress pipe:1` của FFmpeg (không mô phỏng). Output ghi vào `.part.mp4`
//! rồi rename atomic khi FFmpeg thoát thành công — không bao giờ công bố video chưa render xong.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::ffmpeg::command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInput {
    pub path: PathBuf,
    pub start_ms: i64,
    /// Thời lượng đoạn (ms) sau khi trim — đã được kiểm tra hợp lệ.
    pub duration_ms: i64,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderSpec {
    pub segments: Vec<SegmentInput>,
    pub width: i64,
    pub height: i64,
    pub fps: f64,
    pub crf: i64,
    pub preset: String,
    pub audio_bitrate_kbps: i64,
    pub encoder: String,
    pub threads: usize,
}

impl RenderSpec {
    pub fn total_duration_ms(&self) -> i64 {
        self.segments.iter().map(|s| s.duration_ms).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.segments.is_empty() {
            return Err("cần ít nhất một đoạn nguồn".into());
        }
        if self.segments.len() > 50 {
            return Err("tối đa 50 đoạn mỗi lần render".into());
        }
        if !(16..=7680).contains(&self.width) || !(16..=4320).contains(&self.height) {
            return Err("độ phân giải ngoài phạm vi hỗ trợ (16–7680 x 16–4320)".into());
        }
        if self.width % 2 != 0 || self.height % 2 != 0 {
            return Err("độ phân giải H.264 yuv420p phải là số chẵn".into());
        }
        if !(1.0..=120.0).contains(&self.fps) {
            return Err("fps phải trong khoảng 1–120".into());
        }
        if !(0..=51).contains(&self.crf) {
            return Err("crf phải trong khoảng 0–51".into());
        }
        if !matches!(self.preset.as_str(), "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium" | "slow") {
            return Err("preset không hợp lệ".into());
        }
        if !(32..=512).contains(&self.audio_bitrate_kbps) {
            return Err("bitrate audio phải trong khoảng 32–512 kbps".into());
        }
        if self.segments.iter().any(|s| s.duration_ms < 100 || s.start_ms < 0) {
            return Err("mỗi đoạn phải dài tối thiểu 0,1 giây và bắt đầu ≥ 0".into());
        }
        Ok(())
    }
}

fn secs(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// Xây danh sách tham số FFmpeg (tách riêng để unit test).
pub fn build_args(spec: &RenderSpec, out_part: &Path) -> Vec<String> {
    let mut a: Vec<String> =
        ["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1", "-nostats"].iter().map(|s| s.to_string()).collect();
    for s in &spec.segments {
        if s.start_ms > 0 {
            a.extend(["-ss".into(), secs(s.start_ms)]);
        }
        a.extend(["-t".into(), secs(s.duration_ms), "-i".into(), s.path.to_string_lossy().into_owned()]);
    }
    let (w, h, fps) = (spec.width, spec.height, spec.fps);
    let mut fc = String::new();
    let mut concat_in = String::new();
    for (i, s) in spec.segments.iter().enumerate() {
        fc.push_str(&format!(
            "[{i}:v:0]scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps},format=yuv420p[v{i}];"
        ));
        if s.has_audio {
            fc.push_str(&format!("[{i}:a:0]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}];"));
        } else {
            fc.push_str(&format!("anullsrc=r=48000:cl=stereo,atrim=duration={},aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}];", secs(s.duration_ms)));
        }
        concat_in.push_str(&format!("[v{i}][a{i}]"));
    }
    fc.push_str(&format!("{concat_in}concat=n={}:v=1:a=1[vout][aout]", spec.segments.len()));
    a.extend(["-filter_complex".into(), fc, "-map".into(), "[vout]".into(), "-map".into(), "[aout]".into()]);
    a.extend(["-c:v".into(), spec.encoder.clone()]);
    let crf = spec.crf.to_string();
    match spec.encoder.as_str() {
        "libx264" => a.extend(["-preset".into(), spec.preset.clone(), "-crf".into(), crf, "-threads".into(), spec.threads.to_string()]),
        "h264_nvenc" => a.extend(["-preset".into(), "p5".into(), "-rc".into(), "vbr".into(), "-cq".into(), crf, "-b:v".into(), "0".into()]),
        "h264_qsv" => a.extend(["-preset".into(), "medium".into(), "-global_quality".into(), crf]),
        "h264_amf" => a.extend(["-quality".into(), "balanced".into(), "-rc".into(), "cqp".into(), "-qp_i".into(), crf.clone(), "-qp_p".into(), crf]),
        _ => {
            // videotoolbox / mf: dùng bitrate mục tiêu theo độ phân giải
            let kbps = ((w * h) as f64 * fps * 0.1 / 1000.0).clamp(1500.0, 40000.0) as i64;
            a.extend(["-b:v".into(), format!("{kbps}k")]);
        }
    }
    a.extend([
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{}k", spec.audio_bitrate_kbps),
        "-movflags".into(),
        "+faststart".into(),
        "-f".into(),
        "mp4".into(),
        out_part.to_string_lossy().into_owned(),
    ]);
    a
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderUpdate {
    Progress { ratio: f64, eta_seconds: Option<i64>, speed: Option<String> },
    Log(String),
}

#[derive(Debug)]
pub enum RenderOutcome {
    Done,
    Canceled,
    Failed(String),
}

/// Đường dẫn file tạm tương ứng với output cuối cùng.
pub fn part_path(final_path: &Path) -> PathBuf {
    let stem = final_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    final_path.with_file_name(format!(".{stem}.part.mp4"))
}

pub fn parse_progress_line(line: &str, total_ms: i64, state: &mut (i64, Option<String>)) -> Option<RenderUpdate> {
    let (k, v) = line.split_once('=')?;
    match k.trim() {
        "out_time_us" | "out_time_ms" => {
            // cả hai khoá đều tính bằng micro giây trong FFmpeg hiện đại
            if let Ok(us) = v.trim().parse::<i64>() {
                state.0 = (us / 1000).max(0);
            }
            None
        }
        "speed" => {
            let s = v.trim();
            state.1 = (s != "N/A").then(|| s.to_string());
            None
        }
        "progress" => {
            let ratio = if total_ms > 0 { (state.0 as f64 / total_ms as f64).clamp(0.0, 1.0) } else { 0.0 };
            let speed_x = state.1.as_deref().and_then(|s| s.trim_end_matches('x').trim().parse::<f64>().ok()).filter(|x| *x > 0.0);
            let eta = speed_x.map(|x| (((total_ms - state.0).max(0) as f64 / 1000.0) / x).round() as i64);
            let ratio = if v.trim() == "end" { 1.0 } else { ratio };
            Some(RenderUpdate::Progress { ratio, eta_seconds: eta, speed: state.1.clone() })
        }
        _ => None,
    }
}

/// Chạy FFmpeg. Hủy được qua `cancel` (kill tiến trình, xoá file tạm).
pub async fn run(ffmpeg: &Path, spec: &RenderSpec, final_path: &Path, updates: UnboundedSender<RenderUpdate>, cancel: CancellationToken) -> RenderOutcome {
    let part = part_path(final_path);
    let args = build_args(spec, &part);
    let total = spec.total_duration_ms();
    let _ = updates.send(RenderUpdate::Log(format!("ffmpeg {}", args.join(" "))));
    let mut cmd = command(ffmpeg);
    cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return RenderOutcome::Failed(format!("không khởi chạy được FFmpeg: {e}")),
    };
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let tx = updates.clone();
    let progress_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut state = (0i64, None);
        while let Ok(Some(l)) = lines.next_line().await {
            if let Some(u) = parse_progress_line(&l, total, &mut state) {
                let _ = tx.send(u);
            }
        }
    });
    let tx2 = updates.clone();
    let stderr_task = tokio::spawn(async move {
        let mut tail: Vec<String> = vec![];
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            let _ = tx2.send(RenderUpdate::Log(l.clone()));
            tail.push(l);
            if tail.len() > 20 {
                tail.remove(0);
            }
        }
        tail
    });
    let status = tokio::select! {
        s = child.wait() => Some(s),
        _ = cancel.cancelled() => {
            let _ = child.kill().await;
            None
        }
    };
    let _ = progress_task.await;
    let tail = stderr_task.await.unwrap_or_default();
    match status {
        None => {
            let _ = tokio::fs::remove_file(&part).await;
            RenderOutcome::Canceled
        }
        Some(Ok(s)) if s.success() => match tokio::fs::rename(&part, final_path).await {
            Ok(()) => RenderOutcome::Done,
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                RenderOutcome::Failed(format!("không hoàn tất file output: {e}"))
            }
        },
        Some(Ok(s)) => {
            let _ = tokio::fs::remove_file(&part).await;
            let msg = tail.last().cloned().unwrap_or_else(|| format!("FFmpeg thoát mã {:?}", s.code()));
            RenderOutcome::Failed(msg)
        }
        Some(Err(e)) => {
            let _ = tokio::fs::remove_file(&part).await;
            RenderOutcome::Failed(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(enc: &str) -> RenderSpec {
        RenderSpec {
            segments: vec![
                SegmentInput { path: "a.mp4".into(), start_ms: 1500, duration_ms: 2000, has_audio: true },
                SegmentInput { path: "b.mov".into(), start_ms: 0, duration_ms: 1000, has_audio: false },
            ],
            width: 1280,
            height: 720,
            fps: 30.0,
            crf: 23,
            preset: "veryfast".into(),
            audio_bitrate_kbps: 160,
            encoder: enc.into(),
            threads: 4,
        }
    }

    #[test]
    fn args_trim_concat_and_silent_audio() {
        let a = build_args(&spec("libx264"), Path::new("out.part.mp4"));
        let joined = a.join(" ");
        assert!(joined.contains("-ss 1.500 -t 2.000 -i a.mp4"));
        assert!(joined.contains("-t 1.000 -i b.mov"));
        assert!(!joined.contains("-ss 0.000"));
        assert!(joined.contains("anullsrc=r=48000"));
        assert!(joined.contains("concat=n=2:v=1:a=1[vout][aout]"));
        assert!(joined.contains("-c:v libx264 -preset veryfast -crf 23 -threads 4"));
        assert!(joined.contains("-movflags +faststart"));
        assert_eq!(a.last().unwrap(), "out.part.mp4");
    }

    #[test]
    fn hardware_encoder_args() {
        assert!(build_args(&spec("h264_nvenc"), Path::new("o")).join(" ").contains("-cq 23"));
        assert!(build_args(&spec("h264_videotoolbox"), Path::new("o")).join(" ").contains("-b:v"));
    }

    #[test]
    fn validation() {
        assert!(spec("libx264").validate().is_ok());
        let mut s = spec("libx264");
        s.width = 1281;
        assert!(s.validate().is_err());
        let mut s = spec("libx264");
        s.preset = "; rm -rf /".into();
        assert!(s.validate().is_err());
        let mut s = spec("libx264");
        s.segments.clear();
        assert!(s.validate().is_err());
    }

    #[test]
    fn progress_parsing_and_eta() {
        let mut st = (0, None);
        assert!(parse_progress_line("out_time_us=1500000", 3000, &mut st).is_none());
        assert!(parse_progress_line("speed=1.5x", 3000, &mut st).is_none());
        match parse_progress_line("progress=continue", 3000, &mut st) {
            Some(RenderUpdate::Progress { ratio, eta_seconds, speed }) => {
                assert!((ratio - 0.5).abs() < 1e-9);
                assert_eq!(eta_seconds, Some(1));
                assert_eq!(speed.as_deref(), Some("1.5x"));
            }
            o => panic!("{o:?}"),
        }
        assert!(matches!(parse_progress_line("progress=end", 3000, &mut st), Some(RenderUpdate::Progress { ratio, .. }) if ratio == 1.0));
    }

    #[test]
    fn part_path_is_hidden_sibling() {
        let p = part_path(Path::new("/x/y/My Video.mp4"));
        assert_eq!(p, Path::new("/x/y/.My Video.part.mp4"));
    }
}

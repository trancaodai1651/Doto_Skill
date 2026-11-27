//! Tìm và xác minh FFmpeg/ffprobe. Doto Skill KHÔNG phân phối FFmpeg kèm installer
//! (tránh rủi ro giấy phép GPL/nonfree); người dùng cài từ nguồn chính thức hoặc trỏ đường dẫn.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegInfo {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub version: String,
    pub license_flags: Vec<String>,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Vị trí cài đặt thông dụng ngoài PATH.
fn common_locations(name: &str) -> Vec<PathBuf> {
    let mut v = vec![];
    let file = exe(name);
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            v.push(PathBuf::from(&local).join("Microsoft").join("WinGet").join("Links").join(&file));
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            v.push(PathBuf::from(&home).join("scoop").join("shims").join(&file));
        }
        v.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin").join(&file));
        v.push(PathBuf::from(r"C:\ffmpeg\bin").join(&file));
    }
    #[cfg(unix)]
    {
        v.push(PathBuf::from("/opt/homebrew/bin").join(&file));
        v.push(PathBuf::from("/usr/local/bin").join(&file));
        v.push(PathBuf::from("/opt/local/bin").join(&file));
        v.push(PathBuf::from("/usr/bin").join(&file));
    }
    v
}

pub fn locate(name: &str, configured: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = configured.filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(c);
        return p.is_file().then_some(p);
    }
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    common_locations(name).into_iter().find(|p| p.is_file())
}

/// Chạy `ffmpeg -version`, đọc version và cờ cấu hình liên quan giấy phép.
pub async fn verify(ffmpeg: &Path, ffprobe: &Path) -> anyhow::Result<FfmpegInfo> {
    let out = run_with_timeout(ffmpeg, &["-hide_banner", "-version"], Duration::from_secs(10)).await?;
    let first = out.lines().next().unwrap_or_default();
    anyhow::ensure!(first.starts_with("ffmpeg version"), "file không phải FFmpeg hợp lệ: {}", ffmpeg.display());
    let version = first.trim_start_matches("ffmpeg version").split_whitespace().next().unwrap_or("?").to_string();
    let mut flags = vec![];
    for (needle, flag) in [("--enable-gpl", "gpl"), ("--enable-nonfree", "nonfree"), ("--enable-version3", "version3")] {
        if out.contains(needle) {
            flags.push(flag.to_string());
        }
    }
    let probe_out = run_with_timeout(ffprobe, &["-hide_banner", "-version"], Duration::from_secs(10)).await?;
    anyhow::ensure!(probe_out.starts_with("ffprobe version"), "file không phải ffprobe hợp lệ: {}", ffprobe.display());
    Ok(FfmpegInfo { ffmpeg_path: ffmpeg.into(), ffprobe_path: ffprobe.into(), version, license_flags: flags })
}

pub async fn discover(ffmpeg_cfg: Option<&str>, ffprobe_cfg: Option<&str>) -> anyhow::Result<FfmpegInfo> {
    let ffmpeg = locate("ffmpeg", ffmpeg_cfg).ok_or_else(|| anyhow::anyhow!("không tìm thấy ffmpeg (PATH hoặc đường dẫn cấu hình)"))?;
    let ffprobe = locate("ffprobe", ffprobe_cfg)
        .or_else(|| ffmpeg.parent().map(|d| d.join(exe("ffprobe"))).filter(|p| p.is_file()))
        .ok_or_else(|| anyhow::anyhow!("không tìm thấy ffprobe"))?;
    verify(&ffmpeg, &ffprobe).await
}

pub fn command(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

pub async fn run_with_timeout(program: &Path, args: &[&str], timeout: Duration) -> anyhow::Result<String> {
    let mut cmd = command(program);
    cmd.args(args);
    let out = tokio::time::timeout(timeout, cmd.output()).await.map_err(|_| anyhow::anyhow!("quá thời gian chờ {}", program.display()))??;
    anyhow::ensure!(
        out.status.success(),
        "{} thoát mã {:?}: {}",
        program.display(),
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

//! Tiện ích khởi tạo tiến trình con an toàn, dùng chung cho mọi crate (`system`, `media`, `skills`, `integrations`).
//! - Không bao giờ chạy qua shell tuỳ ý.
//! - Tự động đặt cờ `CREATE_NO_WINDOW` (`0x0800_0000`) trên Windows để không bật cửa sổ console phụ.

use std::ffi::OsStr;
use std::process::Stdio;

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Áp dụng cấu hình nền tảng (ẩn cửa sổ console trên Windows) cho `std::process::Command`.
pub fn configure_std_command(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Khởi tạo `std::process::Command` chạy nền với `stdin(Stdio::null())` và ẩn cửa sổ console trên Windows.
pub fn headless_std_command(program: impl AsRef<OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.stdin(Stdio::null());
    configure_std_command(&mut cmd);
    cmd
}

/// Áp dụng cấu hình nền tảng (ẩn cửa sổ console trên Windows + `kill_on_drop(true)`) cho `tokio::process::Command`.
pub fn configure_tokio_command(cmd: &mut tokio::process::Command) {
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Khởi tạo `tokio::process::Command` chạy nền với `stdin(Stdio::null())`, `kill_on_drop(true)` và ẩn cửa sổ console.
pub fn headless_tokio_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.stdin(Stdio::null());
    configure_tokio_command(&mut cmd);
    cmd
}

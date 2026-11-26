#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::{Manager, RunEvent, Url, WindowEvent};

struct StudioServer(Mutex<Option<Child>>);

fn studio_port() -> u16 {
    std::env::var("DOTO_SKILL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .unwrap_or(4320)
}

fn plain_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p,
    }
}

fn studio_up() -> bool {
    let port = studio_port();
    let Ok(mut stream) = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(350)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(format!("GET /api/doctor HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes())
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 16];
    matches!(stream.read(&mut head), Ok(n) if n >= 12 && head.starts_with(b"HTTP/") && &head[9..12] == b"200")
}

fn node_binary(res: &PathBuf) -> PathBuf {
    let bundled = res
        .join("node")
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    if bundled.exists() {
        bundled
    } else {
        PathBuf::from("node")
    }
}

fn resolve_app_dir(res: &PathBuf) -> PathBuf {
    let bundled = res.join("app");
    if bundled.join("bin").join("cli.mjs").exists() {
        return bundled;
    }
    // Hỗ trợ khi chạy `tauri dev` trực tiếp từ thư mục mã nguồn
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("bin").join("cli.mjs").exists() {
            return cwd;
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("bin").join("cli.mjs").exists() {
                return parent.to_path_buf();
            }
        }
    }
    bundled
}

fn start_server(res: &PathBuf, log_dir: &PathBuf) -> std::io::Result<Child> {
    let app_dir = resolve_app_dir(res);
    let _ = fs::create_dir_all(log_dir);
    let log = fs::File::create(log_dir.join("studio-server.log"))?;
    let mut cmd = Command::new(node_binary(res));
    cmd.arg(app_dir.join("bin").join("cli.mjs"))
        .current_dir(&app_dir)
        .env("NODE_ENV", "production")
        .env("DOTO_SKILL_PORT", studio_port().to_string())
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    cmd.spawn()
}

fn stop_server(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn stop_owned_server(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<StudioServer>() {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(mut c) = guard.take() {
                stop_server(&mut c);
            }
        }
    }
}

fn json_quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"Lỗi không xác định\"".to_string())
}

fn show_error(win: &tauri::WebviewWindow, msg: &str) {
    let _ = win.eval(&format!("window.studioFailed && window.studioFailed({})", json_quote(msg)));
}

fn main() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|app| {
            let res = plain_path(app.path().resource_dir()?);
            let log_dir = app.path().home_dir()?.join(".doto-skill");
            let mut child = None;
            let mut start_err = None;

            if !studio_up() {
                match start_server(&res, &log_dir) {
                    Ok(c) => child = Some(c),
                    Err(e) => start_err = Some(format!("Không thể khởi chạy máy chủ Doto Skill: {e}")),
                }
            }

            app.manage(StudioServer(Mutex::new(child)));
            let win = app.get_webview_window("main").expect("main window");
            let handle = app.handle().clone();

            thread::spawn(move || {
                let port = studio_port();
                if let Some(msg) = start_err {
                    show_error(&win, &msg);
                    return;
                }
                for _ in 0..120 {
                    if studio_up() {
                        let _ = win.navigate(Url::parse(&format!("http://localhost:{port}/")).expect("valid url"));
                        return;
                    }
                    if let Some(state) = handle.try_state::<StudioServer>() {
                        if let Ok(mut g) = state.0.lock() {
                            if let Some(c) = g.as_mut() {
                                if let Ok(Some(code)) = c.try_wait() {
                                    show_error(
                                        &win,
                                        &format!(
                                            "Máy chủ dừng sớm ({code}). Xem nhật ký tại: {}",
                                            log_dir.join("studio-server.log").display()
                                        ),
                                    );
                                    return;
                                }
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(250));
                }
                show_error(
                    &win,
                    &format!(
                        "Máy chủ không phản hồi trên cổng {port}. Nhật ký: {}",
                        log_dir.join("studio-server.log").display()
                    ),
                );
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Lỗi khởi tạo ứng dụng Tauri Doto Skill")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                stop_owned_server(app);
            }
        });
}

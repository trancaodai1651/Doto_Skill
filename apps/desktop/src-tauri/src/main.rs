//! Doto Skill desktop (Tauri v2).
//! - Chạy máy chủ Doto Skill trong cùng tiến trình (listener Control 127.0.0.1 + Public tuỳ chọn).
//! - Cửa sổ chính mở giao diện web qua listener Control bằng token một lần (URL fragment, không qua mạng).
//! - Khay hệ thống: mở cửa sổ, bật/tắt web server, thoát. Đóng cửa sổ → chạy nền nếu bật "Chạy nền".
//! - Khởi động cùng hệ điều hành theo cài đặt `launchAtLogin` (tauri-plugin-autostart).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

use doto_core::settings;
use doto_server::{AppState, Options};

struct Shared {
    state: Arc<AppState>,
    control_url: String,
}

fn web_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DOTO_WEB_ROOT") {
        return Some(PathBuf::from(p));
    }
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("web");
        if p.join("index.html").is_file() {
            return Some(p);
        }
    }
    // chạy dev từ mã nguồn
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    dev.join("index.html").is_file().then_some(dev)
}

fn open_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let Some(shared) = app.try_state::<Shared>() else { return };
    let token = shared.state.issue_desktop_token();
    let url = format!("{}/#desktop-token={token}", shared.control_url);
    let Ok(parsed) = url.parse() else { return };
    let res = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
        .title("Doto Skill")
        .inner_size(1360.0, 880.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .build();
    if let Err(e) = res {
        tracing::error!(error = %e, "không mở được cửa sổ");
    }
}

async fn toggle_server(app: AppHandle) {
    let Some(shared) = app.try_state::<Shared>() else { return };
    let st = shared.state.clone();
    if st.public.is_running() {
        st.tunnel.stop().await;
        st.stop_public().await;
    } else if let Err(e) = st.start_public().await {
        tracing::warn!(error = %e, "không bật được web server từ khay");
    }
}

/// Đồng bộ trạng thái autostart của OS với cài đặt `launchAtLogin` (UI web ghi vào DB).
fn spawn_autostart_sync(app: AppHandle, st: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut last: Option<bool> = None;
        loop {
            let s: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
            if last != Some(s.launch_at_login) {
                let al = app.autolaunch();
                let cur = al.is_enabled().unwrap_or(false);
                let r = match (s.launch_at_login, cur) {
                    (true, false) => al.enable(),
                    (false, true) => al.disable(),
                    _ => Ok(()),
                };
                if let Err(e) = r {
                    tracing::warn!(error = %e, "không cập nhật được khởi động cùng hệ điều hành");
                }
                last = Some(s.launch_at_login);
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_main_window(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let root = web_root(&handle);
            if root.is_none() {
                tracing::warn!("không tìm thấy giao diện web (resource web/) — chỉ API hoạt động");
            }
            let (state, control) = tauri::async_runtime::block_on(async move {
                let st = AppState::init(Options { web_root: root, ..Default::default() }).await?;
                let addr = st.start_control(0).await?;
                st.autostart().await;
                anyhow::Ok((st, format!("http://{addr}")))
            })?;
            app.manage(Shared { state: state.clone(), control_url: control });
            spawn_autostart_sync(handle.clone(), state);

            let open = MenuItem::with_id(app, "open", "Mở Doto Skill", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Bật / tắt web server", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Thoát (dừng máy chủ)", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &PredefinedMenuItem::separator(app)?, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main").tooltip("Doto Skill").menu(&menu).show_menu_on_left_click(false);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, ev| match ev.id().as_ref() {
                "open" => open_main_window(app),
                "toggle" => {
                    tauri::async_runtime::spawn(toggle_server(app.clone()));
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, ev| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                    open_main_window(tray.app_handle());
                }
            })
            .build(app)?;

            let background = std::env::args().any(|a| a == "--background");
            if !background {
                open_main_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, ev| {
            if let WindowEvent::CloseRequested { api, .. } = ev {
                let app = window.app_handle();
                let Some(shared) = app.try_state::<Shared>() else { return };
                let st = shared.state.clone();
                let bg = tauri::async_runtime::block_on(async move {
                    settings::get::<settings::AppSettings>(&st.db, settings::KEY_APP).await.map(|s| s.run_in_background).unwrap_or(false)
                });
                if bg {
                    // Chạy nền: ẩn cửa sổ, máy chủ tiếp tục phục vụ thiết bị khác
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    app.exit(0);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("không khởi tạo được ứng dụng Tauri");

    app.run(|app, ev| {
        if let RunEvent::ExitRequested { .. } | RunEvent::Exit = ev {
            if let Some(shared) = app.try_state::<Shared>() {
                let st = shared.state.clone();
                tauri::async_runtime::block_on(async move { st.shutdown().await });
            }
        }
    });
}

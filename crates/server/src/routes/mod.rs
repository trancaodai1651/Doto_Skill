//! Toàn bộ route `/api/*` (xem `docs/API.md`), được tổ chức thành các module chuyên trách:
//! - [`auth_system`]: Xác thực, người dùng, thiết bị, mã ghép nối, phần cứng, tài nguyên, cài đặt, sao lưu/khôi phục SQLite, SSE & nhật ký kiểm toán.
//! - [`skills_tools`]: Quản lý AI Skills, nguồn GitHub/thư mục, kết nối GitHub OAuth/`gh` CLI, cài đặt 4 công cụ CLI, cấu hình máy chủ Public & Cloudflare Tunnel.
//! - [`media_jobs`]: Thư viện video nguồn/xuất, HTTP Range streaming, tải xuống, chia sẻ có thời hạn và hàng đợi job render/AI Studio.
//!
//! Bản Desktop App và bản Web có đầy đủ chức năng quản trị ngang nhau cho Owner đã xác thực
//! (thực thi trên phần cứng máy chủ của bản App qua allowlist an toàn, không nhận lệnh shell/FS tuỳ ý).
//! Chỉ `/api/auth/desktop` dành riêng cho token khởi động một lần của cửa sổ Tauri.

pub mod auth_system;
pub mod media_jobs;
pub mod skills_tools;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_core::audit;

use crate::auth::{self, Auth, ListenerKind};
use crate::AppState;

pub(crate) type St = State<Arc<AppState>>;
pub(crate) type Peer = ConnectInfo<SocketAddr>;

#[derive(Deserialize)]
pub(crate) struct LimitQ {
    pub limit: Option<u32>,
}

pub fn api_router(kind: ListenerKind) -> Router<Arc<AppState>> {
    let mut r = Router::new()
        .route("/api/health", get(auth_system::health))
        .route("/api/auth/me", get(auth_system::me))
        .route("/api/auth/setup", post(auth_system::setup_owner))
        .route("/api/auth/login", post(auth_system::login))
        .route("/api/auth/pair", post(auth_system::pair))
        .route("/api/auth/logout", post(auth_system::logout))
        .route("/api/auth/ticket", post(auth_system::issue_ticket))
        .route("/api/users", get(auth_system::list_users).post(auth_system::create_user))
        .route("/api/users/{id}", delete(auth_system::delete_user))
        .route("/api/users/{id}/password", post(auth_system::set_user_password))
        .route("/api/devices", get(auth_system::list_devices))
        .route("/api/devices/{id}", delete(auth_system::revoke_device))
        .route("/api/pairing", post(auth_system::create_pairing))
        .route("/api/system/hardware", get(auth_system::get_hardware))
        .route("/api/system/hardware/probe", post(auth_system::probe_hardware))
        .route("/api/system/metrics", get(auth_system::get_metrics))
        .route("/api/system/ffmpeg", get(auth_system::get_ffmpeg))
        .route("/api/system/ffmpeg/probe", post(auth_system::probe_ffmpeg))
        .route("/api/system/ffmpeg/benchmark", post(auth_system::benchmark_ffmpeg))
        .route("/api/system/backups", get(auth_system::list_backups))
        .route("/api/system/backup", post(auth_system::backup))
        .route("/api/system/restore", post(auth_system::restore_backup))
        .route("/api/settings/resources", get(auth_system::get_resources).put(auth_system::put_resources))
        .route("/api/settings", get(auth_system::get_settings).put(auth_system::put_settings))
        .route("/api/onboarding", get(auth_system::get_onboarding))
        .route("/api/onboarding/complete", post(auth_system::complete_onboarding))
        .route("/api/events", get(auth_system::events))
        .route("/api/audit", get(auth_system::list_audit))
        .route("/api/skills", get(skills_tools::list_skills))
        .route("/api/skills/{id}", get(skills_tools::skill_detail))
        .route("/api/skills/{id}/install", post(skills_tools::install_skill))
        .route("/api/skills/{id}/install/{provider}", delete(skills_tools::uninstall_skill))
        .route("/api/skill-sources", get(skills_tools::list_sources))
        .route("/api/skill-sources/folder", post(skills_tools::add_folder_source))
        .route("/api/skill-sources/github", post(skills_tools::add_github_source))
        .route("/api/skill-sources/{id}", patch(skills_tools::patch_source).delete(skills_tools::delete_source))
        .route("/api/skill-sources/{id}/rescan", post(skills_tools::rescan_source))
        .route("/api/skill-sources/{id}/check-update", post(skills_tools::check_update))
        .route("/api/skill-sources/{id}/apply-update", post(skills_tools::apply_update))
        .route("/api/github/status", get(skills_tools::github_status))
        .route("/api/github/device/start", post(skills_tools::github_device_start))
        .route("/api/github/device/poll", post(skills_tools::github_device_poll))
        .route("/api/github/use-gh", post(skills_tools::github_use_gh))
        .route("/api/github/logout", post(skills_tools::github_logout))
        .route("/api/tools", get(skills_tools::list_tools))
        .route("/api/tools/{id}/refresh", post(skills_tools::refresh_tool))
        .route("/api/tools/{id}/install", post(skills_tools::install_tool))
        .route("/api/tools/{id}/diagnostics", get(skills_tools::tool_diagnostics))
        .route("/api/server/status", get(skills_tools::server_status))
        .route("/api/server/config", get(skills_tools::get_server_config).put(skills_tools::put_server_config))
        .route("/api/server/start", post(skills_tools::server_start))
        .route("/api/server/stop", post(skills_tools::server_stop))
        .route("/api/server/restart", post(skills_tools::server_restart))
        .route("/api/server/qr", get(skills_tools::server_qr))
        .route("/api/server/firewall-guide", get(skills_tools::firewall_guide))
        .route("/api/tunnel/status", get(skills_tools::tunnel_status))
        .route("/api/tunnel/config", put(skills_tools::put_tunnel_config))
        .route("/api/tunnel/start", post(skills_tools::tunnel_start))
        .route("/api/tunnel/stop", post(skills_tools::tunnel_stop))
        .route("/api/tunnel/token", delete(skills_tools::delete_tunnel_token))
        .route("/api/videos", get(media_jobs::list_videos))
        .route("/api/videos/scan", post(media_jobs::scan_videos))
        .route("/api/videos/{id}", get(media_jobs::get_video).patch(media_jobs::patch_video).delete(media_jobs::delete_video))
        .route("/api/videos/{id}/thumbnail", get(media_jobs::video_thumbnail))
        .route("/api/videos/{id}/stream", get(media_jobs::video_stream))
        .route("/api/videos/{id}/download", get(media_jobs::video_download))
        .route("/api/videos/{id}/share", post(media_jobs::create_share))
        .route("/api/videos/{id}/shares", get(media_jobs::list_shares))
        .route("/api/shares/{id}", delete(media_jobs::revoke_share))
        .route("/api/jobs", get(media_jobs::list_jobs))
        .route("/api/jobs/render", post(media_jobs::create_render))
        .route("/api/jobs/{id}", get(media_jobs::job_detail))
        .route("/api/jobs/{id}/cancel", post(media_jobs::cancel_job))
        .route("/api/jobs/{id}/retry", post(media_jobs::retry_job))
        .route("/s/{token}", get(media_jobs::share_stream));
    if kind == ListenerKind::Control {
        r = r.route("/api/auth/desktop", post(auth_system::desktop_login));
    }
    r
}

pub(crate) fn ok() -> Json<Value> {
    Json(json!({ "ok": true }))
}

pub(crate) fn ua(h: &HeaderMap) -> Option<String> {
    h.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(String::from)
}

pub(crate) async fn audit_ev(st: &AppState, a: &Auth, action: &str, target: Option<String>, detail: Option<Value>) {
    audit::record(&st.db, a.username.clone(), action, target, detail, a.ip.clone()).await;
}

pub(crate) fn rate_key(peer: &Peer, h: &HeaderMap, scope: &str) -> String {
    format!("{scope}:{}", auth::client_ip(Some(peer.0), h).map(|i| i.to_string()).unwrap_or_default())
}

pub(crate) fn qr_svg(data: &str) -> Option<String> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    Some(code.render::<qrcode::render::svg::Color>().min_dimensions(220, 220).quiet_zone(true).build())
}

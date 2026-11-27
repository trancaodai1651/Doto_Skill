//! Route quản lý AI Skills, nguồn GitHub/thư mục, kết nối GitHub OAuth/`gh` CLI,
//! công cụ AI CLI (`agy`/`codex`/`claude`/`cloudflared`), máy chủ Public và Cloudflare Tunnel.

use std::time::{Duration, Instant};

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_core::settings;

use crate::auth::{self, Auth, ListenerKind};
use crate::error::{ApiError, ApiResult};
use crate::public::{self, ServerConfig};
use crate::AppState;

use super::{audit_ev, ok, qr_svg, St};

// ======================= Skills =======================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillQ {
    q: Option<String>,
    source_id: Option<String>,
}

pub(crate) async fn list_skills(State(st): St, a: Auth, Query(q): Query<SkillQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.list_skills(q.q, q.source_id).await?)))
}

pub(crate) async fn skill_detail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.detail(&id).await?)))
}

pub(crate) async fn list_sources(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.list_sources().await?)))
}

#[derive(Deserialize)]
pub(crate) struct FolderReq {
    path: String,
    watch: Option<bool>,
}

pub(crate) async fn add_folder_source(State(st): St, a: Auth, Json(b): Json<FolderReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.add_folder(&b.path, b.watch.unwrap_or(false)).await?;
    audit_ev(&st, &a, "skills.source.add_folder", Some(s.id.clone()), Some(json!({ "skills": s.skill_count }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubReq {
    url: String,
    #[serde(rename = "ref")]
    git_ref: Option<String>,
}

pub(crate) async fn add_github_source(State(st): St, a: Auth, Json(b): Json<GithubReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.add_github(&b.url, b.git_ref.as_deref()).await?;
    audit_ev(&st, &a, "skills.source.add_github", Some(s.id.clone()), Some(json!({ "repo": s.location, "sha": s.current_sha, "private": s.private }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
pub(crate) struct SourcePatch {
    enabled: Option<bool>,
    watch: Option<bool>,
    pinned: Option<bool>,
}

pub(crate) async fn patch_source(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<SourcePatch>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.set_flags(&id, b.enabled, b.watch, b.pinned).await?;
    audit_ev(&st, &a, "skills.source.update", Some(id), Some(json!({ "enabled": b.enabled, "watch": b.watch, "pinned": b.pinned }))).await;
    Ok(Json(json!(s)))
}

pub(crate) async fn delete_source(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.skills.remove(&id).await?;
    audit_ev(&st, &a, "skills.source.remove", Some(id), None).await;
    Ok(ok())
}

pub(crate) async fn rescan_source(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.rescan(&id).await?)))
}

pub(crate) async fn check_update(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.check_update(&id).await?)))
}

#[derive(Deserialize)]
pub(crate) struct ApplyReq {
    sha: String,
}

pub(crate) async fn apply_update(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<ApplyReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.apply_update(&id, &b.sha).await?;
    audit_ev(&st, &a, "skills.source.apply_update", Some(id), Some(json!({ "sha": b.sha }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallSkillReq {
    provider: String,
    confirm: bool,
    allow_scripts: Option<bool>,
}

pub(crate) async fn install_skill(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<InstallSkillReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let target = st.skills.install(&id, &b.provider, b.confirm, b.allow_scripts.unwrap_or(false)).await?;
    audit_ev(&st, &a, "skills.install", Some(id), Some(json!({ "provider": b.provider, "allowScripts": b.allow_scripts }))).await;
    Ok(Json(json!({ "ok": true, "target": target })))
}

pub(crate) async fn uninstall_skill(State(st): St, a: Auth, Path((id, provider)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.skills.uninstall(&id, &provider).await?;
    audit_ev(&st, &a, "skills.uninstall", Some(id), Some(json!({ "provider": provider }))).await;
    Ok(ok())
}

// ======================= GitHub =======================

pub(crate) async fn github_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let connected = st.secrets.get(doto_skills::GITHUB_TOKEN_KEY).ok().flatten().is_some();
    let row: Option<(Option<String>, String, String)> = st
        .db
        .call(|c| {
            Ok(c.query_row("SELECT login, scopes, method FROM github_connections ORDER BY created_at DESC LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?)
        })
        .await?;
    let (login, scopes, method) = match (connected, row) {
        (true, Some((l, s, m))) => (l, serde_json::from_str::<Vec<String>>(&s).unwrap_or_default(), Some(m)),
        _ => (None, vec![], None),
    };
    Ok(Json(json!({
        "connected": connected, "login": login, "scopes": scopes, "method": method,
        "deviceFlowAvailable": st.github_client_id.is_some(),
        "ghCliAvailable": doto_integrations::manager::command(std::path::Path::new("gh")).arg("--version").output().await.map(|o| o.status.success()).unwrap_or(false),
        "vault": st.secrets.backend_name(),
    })))
}

async fn store_github_token(st: &AppState, a: &Auth, token: String, method: &str) -> ApiResult<Value> {
    let client = doto_skills::github::GithubClient::new(Some(token.clone()));
    let (login, scopes) = client.user().await.map_err(|e| ApiError::validation(format!("token GitHub không hợp lệ: {e}")))?;
    st.secrets.set(doto_skills::GITHUB_TOKEN_KEY, &token).map_err(ApiError::internal)?;
    let (l, s, m) = (login.clone(), serde_json::to_string(&scopes).unwrap_or_default(), method.to_string());
    st.db
        .call(move |c| {
            c.execute("DELETE FROM github_connections", [])?;
            c.execute(
                "INSERT INTO github_connections(id,login,scopes,method,vault_key,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![doto_core::new_id(), l, s, m, doto_skills::GITHUB_TOKEN_KEY, doto_core::now_rfc3339()],
            )?;
            Ok(())
        })
        .await?;
    audit_ev(st, a, "github.connect", Some(login.clone()), Some(json!({ "method": method, "scopes": scopes }))).await;
    Ok(json!({ "connected": true, "login": login, "scopes": scopes, "method": method }))
}

pub(crate) async fn github_device_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cid = st.github_client_id.clone().ok_or_else(|| {
        ApiError::conflict(
            "chưa cấu hình GitHub OAuth App (biến môi trường DOTO_GITHUB_CLIENT_ID) — dùng tuỳ chọn \"Dùng gh CLI\" hoặc xem docs/INTEGRATIONS.md",
        )
    })?;
    let dc = st.skills.github_anonymous().device_start(&cid).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "github", e.to_string()))?;
    *st.device_flow.lock().unwrap() = Some((dc.device_code.clone(), Instant::now() + Duration::from_secs(dc.expires_in)));
    Ok(Json(json!({ "userCode": dc.user_code, "verificationUri": dc.verification_uri, "expiresIn": dc.expires_in, "interval": dc.interval })))
}

pub(crate) async fn github_device_poll(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cid = st.github_client_id.clone().ok_or_else(|| ApiError::conflict("chưa cấu hình GitHub OAuth App"))?;
    let (code, exp) = st.device_flow.lock().unwrap().clone().ok_or_else(|| ApiError::validation("chưa bắt đầu Device Flow"))?;
    if Instant::now() > exp {
        return Ok(Json(json!({ "status": "expired" })));
    }
    use doto_skills::github::DevicePoll;
    match st.skills.github_anonymous().device_poll(&cid, &code).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "github", e.to_string()))? {
        DevicePoll::Pending => Ok(Json(json!({ "status": "pending" }))),
        DevicePoll::SlowDown => Ok(Json(json!({ "status": "slow_down" }))),
        DevicePoll::Expired => Ok(Json(json!({ "status": "expired" }))),
        DevicePoll::Denied => Ok(Json(json!({ "status": "denied" }))),
        DevicePoll::Token(t) => {
            *st.device_flow.lock().unwrap() = None;
            let mut v = store_github_token(&st, &a, t, "device").await?;
            v["status"] = json!("connected");
            Ok(Json(v))
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ConfirmReq {
    confirm: bool,
}

pub(crate) async fn github_use_gh(State(st): St, a: Auth, Json(b): Json<ConfirmReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận cho phép đọc token từ gh CLI"));
    }
    let gh = which_gh().ok_or_else(|| ApiError::validation("không tìm thấy gh CLI"))?;
    let out = doto_integrations::manager::command(&gh).args(["auth", "token"]).output().await.map_err(ApiError::internal)?;
    if !out.status.success() {
        return Err(ApiError::validation("gh chưa đăng nhập — chạy `gh auth login` trong terminal"));
    }
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(Json(store_github_token(&st, &a, token, "gh").await?))
}

fn which_gh() -> Option<std::path::PathBuf> {
    let bin = doto_core::paths::exe_name("gh");
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(&bin)).find(|c| c.is_file())).or_else(|| {
        let cands = if cfg!(windows) { vec![r"C:\Program Files\GitHub CLI\gh.exe"] } else { vec!["/opt/homebrew/bin/gh", "/usr/local/bin/gh"] };
        cands.into_iter().map(std::path::PathBuf::from).find(|p| p.is_file())
    })
}

pub(crate) async fn github_logout(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.secrets.delete(doto_skills::GITHUB_TOKEN_KEY).map_err(ApiError::internal)?;
    st.db
        .call(|c| {
            c.execute("DELETE FROM github_connections", [])?;
            Ok(())
        })
        .await?;
    audit_ev(&st, &a, "github.logout", None, None).await;
    Ok(Json(json!({ "ok": true, "note": "Token đã xoá khỏi OS vault. Để thu hồi hẳn, vào GitHub › Settings › Applications." })))
}

// ======================= Tools =======================

pub(crate) async fn list_tools(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cached = st.tools.cached().await;
    if cached.len() == doto_integrations::providers::all().len() {
        let mut v = cached;
        let order = ["antigravity", "codex", "claude", "cloudflared"];
        v.sort_by_key(|t| order.iter().position(|o| *o == t.id).unwrap_or(9));
        return Ok(Json(json!(v)));
    }
    Ok(Json(json!(st.tools.status_all().await)))
}

pub(crate) async fn refresh_tool(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.tools.status(&id).await.ok_or_else(|| ApiError::not_found("công cụ không tồn tại"))?;
    st.events.emit("tool", &s);
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallToolReq {
    method_id: String,
    confirm: bool,
}

pub(crate) async fn install_tool(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<InstallToolReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi cài phần mềm"));
    }
    let job = st.tools.install(&id, &b.method_id).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tools.install", Some(id), Some(json!({ "method": b.method_id, "jobId": job.id }))).await;
    Ok(Json(json!(job)))
}

pub(crate) async fn tool_diagnostics(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let lines = st.tools.diagnostics(&id).await.ok_or_else(|| ApiError::not_found("công cụ không tồn tại"))?;
    Ok(Json(json!({ "lines": lines })))
}

// ======================= Server / tunnel =======================

pub(crate) async fn server_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = st.server_config().await;
    Ok(Json(json!(st.public.status(&cfg))))
}

pub(crate) async fn get_server_config(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.server_config().await)))
}

pub(crate) async fn put_server_config(State(st): St, a: Auth, Json(b): Json<ServerConfig>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    b.validate().map_err(ApiError::validation)?;
    if b.mode == "lan" && !auth::owner_password_set(&st).await {
        return Err(ApiError::validation("cần đặt mật khẩu Owner trước khi chọn chế độ LAN"));
    }
    settings::set(&st.db, settings::KEY_SERVER, &b).await?;
    audit_ev(&st, &a, "server.config", None, Some(json!({ "mode": b.mode, "port": b.port, "allowlist": b.ip_allowlist.len() }))).await;
    Ok(Json(json!(b)))
}

pub(crate) async fn server_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.start_public().await.map_err(ApiError::conflict)?;
    audit_ev(&st, &a, "server.start", None, None).await;
    let cfg = st.server_config().await;
    Ok(Json(json!(st.public.status(&cfg))))
}

pub(crate) async fn server_stop(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    audit_ev(&st, &a, "server.stop", None, None).await;
    let cfg = st.server_config().await;
    if kind == ListenerKind::Public {
        let mut s = st.public.status(&cfg);
        s.running = false;
        s.bind_address = None;
        s.urls.clear();
        let st2 = st.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            st2.tunnel.stop().await;
            st2.stop_public().await;
        });
        return Ok(Json(json!(s)));
    }
    st.tunnel.stop().await;
    st.stop_public().await;
    Ok(Json(json!(st.public.status(&cfg))))
}

pub(crate) async fn server_restart(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = st.server_config().await;
    cfg.validate().map_err(ApiError::validation)?;
    if cfg.mode == "off" {
        return Err(ApiError::conflict("chế độ Off — chọn Localhost hoặc LAN"));
    }
    if cfg.mode == "lan" && !auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("cần đặt mật khẩu Owner trước khi mở truy cập LAN"));
    }
    if kind == ListenerKind::Public {
        audit_ev(&st, &a, "server.restart", None, None).await;
        let current = st.public.status(&cfg);
        let st2 = st.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            st2.stop_public().await;
            if let Err(e) = st2.start_public().await {
                tracing::warn!(error = %e, "khởi động lại public server thất bại");
            }
        });
        return Ok(Json(json!(current)));
    }
    st.stop_public().await;
    st.start_public().await.map_err(ApiError::conflict)?;
    audit_ev(&st, &a, "server.restart", None, None).await;
    Ok(Json(json!(st.public.status(&cfg))))
}

#[derive(Deserialize)]
pub(crate) struct QrQ {
    url: String,
}

pub(crate) async fn server_qr(a: Auth, Query(q): Query<QrQ>) -> ApiResult<Response> {
    a.require_owner()?;
    if q.url.len() > 512 || !(q.url.starts_with("http://") || q.url.starts_with("https://")) {
        return Err(ApiError::validation("URL không hợp lệ"));
    }
    let svg = qr_svg(&q.url).ok_or_else(|| ApiError::validation("không tạo được QR"))?;
    Ok(([(header::CONTENT_TYPE, "image/svg+xml"), (header::CACHE_CONTROL, "no-store")], svg).into_response())
}

pub(crate) async fn firewall_guide(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(public::firewall_guide(st.server_config().await.port)))
}

async fn tunnel_cfg(st: &AppState) -> doto_integrations::TunnelConfig {
    settings::get(&st.db, settings::KEY_TUNNEL).await.unwrap_or_default()
}

pub(crate) async fn tunnel_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = tunnel_cfg(&st).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelReq {
    mode: String,
    hostname: Option<String>,
    auto_start: Option<bool>,
    token: Option<String>,
}

pub(crate) async fn put_tunnel_config(State(st): St, a: Auth, Json(b): Json<TunnelReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = doto_integrations::TunnelConfig {
        mode: b.mode,
        hostname: b.hostname.map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()),
        auto_start: b.auto_start.unwrap_or(false),
    };
    cfg.validate().map_err(ApiError::validation)?;
    if let Some(t) = b.token.filter(|t| !t.trim().is_empty()) {
        st.tunnel.set_token(&t).map_err(|e| ApiError::validation(e.to_string()))?;
        audit_ev(&st, &a, "tunnel.token.set", None, None).await;
    }
    settings::set(&st.db, settings::KEY_TUNNEL, &cfg).await?;
    audit_ev(&st, &a, "tunnel.config", None, Some(json!({ "mode": cfg.mode, "hostname": cfg.hostname, "autoStart": cfg.auto_start }))).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

pub(crate) async fn tunnel_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = tunnel_cfg(&st).await;
    let bin = st.cloudflared_path().ok_or_else(|| ApiError::conflict("chưa cài cloudflared — vào Công cụ AI để cài"))?;
    let port = st.public.local_port().ok_or_else(|| ApiError::conflict("bật web server (Localhost hoặc LAN) trước khi mở tunnel"))?;
    if !auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("cần đặt mật khẩu Owner trước khi xuất bản qua Internet"));
    }
    st.tunnel.start(&cfg, bin, port).await.map_err(|e| ApiError::conflict(e.to_string()))?;
    audit_ev(&st, &a, "tunnel.start", None, Some(json!({ "mode": cfg.mode }))).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

pub(crate) async fn tunnel_stop(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.stop().await;
    audit_ev(&st, &a, "tunnel.stop", None, None).await;
    let cfg = tunnel_cfg(&st).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

pub(crate) async fn delete_tunnel_token(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.delete_token().map_err(ApiError::internal)?;
    audit_ev(&st, &a, "tunnel.token.delete", None, None).await;
    Ok(ok())
}

//! Toàn bộ route `/api/*` (xem docs/API.md).
//! Bản Desktop App và bản Web có đầy đủ chức năng quản trị ngang nhau cho Owner đã xác thực
//! (thực thi trên phần cứng máy chủ của bản App qua allowlist an toàn, không nhận lệnh shell/FS tuỳ ý).
//! Chỉ `/api/auth/desktop` dành riêng cho token khởi động một lần của cửa sổ Tauri.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_core::{audit, jobs, settings};
use doto_media::library;

use crate::auth::{self, Auth, ListenerKind, MaybeAuth};
use crate::error::{ApiError, ApiResult};
use crate::public::{self, ServerConfig};
use crate::{stream, AppState};

type St = State<Arc<AppState>>;
type Peer = ConnectInfo<SocketAddr>;

pub fn api_router(kind: ListenerKind) -> Router<Arc<AppState>> {
    let mut r = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/me", get(me))
        .route("/api/auth/setup", post(setup_owner))
        .route("/api/auth/login", post(login))
        .route("/api/auth/pair", post(pair))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/ticket", post(issue_ticket))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{id}", delete(delete_user))
        .route("/api/users/{id}/password", post(set_user_password))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/{id}", delete(revoke_device))
        .route("/api/pairing", post(create_pairing))
        .route("/api/system/hardware", get(get_hardware))
        .route("/api/system/hardware/probe", post(probe_hardware))
        .route("/api/system/metrics", get(get_metrics))
        .route("/api/system/ffmpeg", get(get_ffmpeg))
        .route("/api/system/ffmpeg/probe", post(probe_ffmpeg))
        .route("/api/system/ffmpeg/benchmark", post(benchmark_ffmpeg))
        .route("/api/system/backups", get(list_backups))
        .route("/api/system/backup", post(backup))
        .route("/api/system/restore", post(restore_backup))
        .route("/api/settings/resources", get(get_resources).put(put_resources))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/onboarding", get(get_onboarding))
        .route("/api/onboarding/complete", post(complete_onboarding))
        .route("/api/events", get(events))
        .route("/api/audit", get(list_audit))
        .route("/api/skills", get(list_skills))
        .route("/api/skills/{id}", get(skill_detail))
        .route("/api/skills/{id}/install", post(install_skill))
        .route("/api/skills/{id}/install/{provider}", delete(uninstall_skill))
        .route("/api/skill-sources", get(list_sources))
        .route("/api/skill-sources/folder", post(add_folder_source))
        .route("/api/skill-sources/github", post(add_github_source))
        .route("/api/skill-sources/{id}", patch(patch_source).delete(delete_source))
        .route("/api/skill-sources/{id}/rescan", post(rescan_source))
        .route("/api/skill-sources/{id}/check-update", post(check_update))
        .route("/api/skill-sources/{id}/apply-update", post(apply_update))
        .route("/api/github/status", get(github_status))
        .route("/api/github/device/start", post(github_device_start))
        .route("/api/github/device/poll", post(github_device_poll))
        .route("/api/github/use-gh", post(github_use_gh))
        .route("/api/github/logout", post(github_logout))
        .route("/api/tools", get(list_tools))
        .route("/api/tools/{id}/refresh", post(refresh_tool))
        .route("/api/tools/{id}/install", post(install_tool))
        .route("/api/tools/{id}/diagnostics", get(tool_diagnostics))
        .route("/api/server/status", get(server_status))
        .route("/api/server/config", get(get_server_config).put(put_server_config))
        .route("/api/server/start", post(server_start))
        .route("/api/server/stop", post(server_stop))
        .route("/api/server/restart", post(server_restart))
        .route("/api/server/qr", get(server_qr))
        .route("/api/server/firewall-guide", get(firewall_guide))
        .route("/api/tunnel/status", get(tunnel_status))
        .route("/api/tunnel/config", put(put_tunnel_config))
        .route("/api/tunnel/start", post(tunnel_start))
        .route("/api/tunnel/stop", post(tunnel_stop))
        .route("/api/tunnel/token", delete(delete_tunnel_token))
        .route("/api/videos", get(list_videos))
        .route("/api/videos/scan", post(scan_videos))
        .route("/api/videos/{id}", get(get_video).patch(patch_video).delete(delete_video))
        .route("/api/videos/{id}/thumbnail", get(video_thumbnail))
        .route("/api/videos/{id}/stream", get(video_stream))
        .route("/api/videos/{id}/download", get(video_download))
        .route("/api/videos/{id}/share", post(create_share))
        .route("/api/videos/{id}/shares", get(list_shares))
        .route("/api/shares/{id}", delete(revoke_share))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/render", post(create_render))
        .route("/api/jobs/{id}", get(job_detail))
        .route("/api/jobs/{id}/cancel", post(cancel_job))
        .route("/api/jobs/{id}/retry", post(retry_job))
        .route("/s/{token}", get(share_stream));
    if kind == ListenerKind::Control {
        r = r.route("/api/auth/desktop", post(desktop_login));
    }
    r
}

fn ok() -> Json<Value> {
    Json(json!({ "ok": true }))
}

fn ua(h: &HeaderMap) -> Option<String> {
    h.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(String::from)
}

async fn audit_ev(st: &AppState, a: &Auth, action: &str, target: Option<String>, detail: Option<Value>) {
    audit::record(&st.db, a.username.clone(), action, target, detail, a.ip.clone()).await;
}

fn rate_key(peer: &Peer, h: &HeaderMap, scope: &str) -> String {
    format!("{scope}:{}", auth::client_ip(Some(peer.0), h).map(|i| i.to_string()).unwrap_or_default())
}

// ======================= Auth =======================

async fn health(State(st): St) -> Json<Value> {
    let hostname = st.hardware.read().unwrap().as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
    Json(json!({
        "ok": true,
        "version": crate::VERSION,
        "uptimeSeconds": st.started.elapsed().as_secs(),
        "hostname": hostname,
    }))
}

async fn me_json(st: &AppState, a: Option<&Auth>) -> Value {
    let onboarding: settings::OnboardingState = settings::get(&st.db, settings::KEY_ONBOARDING).await.unwrap_or_default();
    let hostname = st.hardware.read().unwrap().as_ref().map(|h| h.hostname.clone());
    let user = match a {
        Some(a) => {
            let id = a.user_id.clone();
            st.db
                .call(move |c| Ok(c.query_row("SELECT created_at FROM users WHERE id=?1", [id], |r| r.get::<_, String>(0)).optional()?))
                .await
                .ok()
                .flatten()
                .map(|created| json!({ "id": a.user_id, "username": a.username, "role": a.role, "createdAt": created }))
        }
        None => None,
    };
    json!({
        "user": user,
        "csrfToken": a.map(|a| a.csrf.clone()),
        "hostControl": a.map(|a| a.host_control).unwrap_or(false),
        "setupRequired": !auth::owner_password_set(st).await,
        "onboardingCompleted": onboarding.completed,
        "version": crate::VERSION,
        "hostname": hostname,
    })
}

async fn me(State(st): St, MaybeAuth(a): MaybeAuth) -> Json<Value> {
    Json(me_json(&st, a.as_ref()).await)
}

fn with_cookie(cookie: String, body: Value) -> Response {
    ([(header::SET_COOKIE, cookie)], Json(body)).into_response()
}

#[derive(Deserialize)]
struct DesktopReq {
    token: String,
}

async fn desktop_login(State(st): St, peer: Peer, headers: HeaderMap, Json(b): Json<DesktopReq>) -> ApiResult<Response> {
    if !st.rate.check(&rate_key(&peer, &headers, "desktop"), 20, Duration::from_secs(300)) {
        return Err(ApiError::too_many());
    }
    if !st.consume_desktop_token(&b.token) {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "token desktop không hợp lệ hoặc đã dùng"));
    }
    let owner = auth::ensure_local_owner(&st).await?;
    let s = auth::create_session(&st, &owner, "Ứng dụng desktop", ua(&headers), Some("127.0.0.1".into()), true, auth::DESKTOP_TTL).await?;
    let a = Auth { user_id: owner.clone(), username: "owner".into(), role: "owner".into(), host_control: true, session_id: s.id, csrf: s.csrf, ip: None };
    let a = match auth::find_user_by_id(&st, &owner).await? {
        Some(u) => Auth { username: u.username, ..a },
        None => a,
    };
    Ok(with_cookie(auth::session_cookie(ListenerKind::Control, &s.token, auth::DESKTOP_TTL, false), me_json(&st, Some(&a)).await))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupReq {
    username: String,
    password: String,
    auth_mode: Option<String>,
}

async fn setup_owner(
    State(st): St,
    peer: Peer,
    headers: HeaderMap,
    axum::Extension(kind): axum::Extension<ListenerKind>,
    MaybeAuth(a): MaybeAuth,
    Json(b): Json<SetupReq>,
) -> ApiResult<Response> {
    if auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("tài khoản Owner đã được thiết lập"));
    }
    let direct_loopback = peer.0.ip().is_loopback() && headers.get("cf-connecting-ip").is_none() && headers.get("x-forwarded-for").is_none();
    let allowed = a.as_ref().map(|x| x.host_control).unwrap_or(false) || direct_loopback;
    if !allowed {
        return Err(ApiError::forbidden("thiết lập lần đầu chỉ cho phép thực hiện trực tiếp trên máy chủ cục bộ"));
    }
    auth::username_policy(&b.username).map_err(ApiError::validation)?;
    auth::password_policy(&b.password).map_err(ApiError::validation)?;
    let hash = auth::hash_password(&b.password)?;
    let owner_id = match &a {
        Some(existing) if existing.is_owner() => existing.user_id.clone(),
        _ => auth::ensure_local_owner(&st).await?,
    };
    let (uid, uname) = (owner_id.clone(), b.username.clone());
    let res = st
        .db
        .call(move |c| {
            c.execute("UPDATE users SET username=?2, password_hash=?3 WHERE id=?1 AND role='owner'", params![uid, uname, hash])?;
            Ok(())
        })
        .await;
    if let Err(e) = res {
        if e.to_string().contains("UNIQUE") {
            return Err(ApiError::conflict("tên đăng nhập đã tồn tại"));
        }
        return Err(e.into());
    }
    if let Some(existing) = a {
        let updated = Auth { username: b.username.clone(), ..existing };
        audit_ev(&st, &updated, "auth.setup", None, None).await;
        return Ok(Json(me_json(&st, Some(&updated)).await).into_response());
    }
    let ip = auth::client_ip(Some(peer.0), &headers).map(|i| i.to_string());
    let s = auth::create_session(&st, &owner_id, "Trình duyệt cục bộ", ua(&headers), ip.clone(), false, auth::OWNER_TTL).await?;
    let created = Auth { user_id: owner_id, username: b.username, role: "owner".into(), host_control: false, session_id: s.id, csrf: s.csrf, ip };
    audit_ev(&st, &created, "auth.setup", None, None).await;
    let mut body = me_json(&st, Some(&created)).await;
    if b.auth_mode.as_deref() == Some("bearer") {
        body["sessionToken"] = json!(s.token);
        return Ok(Json(body).into_response());
    }
    let secure = auth::is_https(Some(peer.0), &headers);
    Ok(with_cookie(auth::session_cookie(kind, &s.token, auth::OWNER_TTL, secure), body))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginReq {
    username: String,
    password: String,
    device_name: Option<String>,
    auth_mode: Option<String>,
}

async fn login(
    State(st): St,
    peer: Peer,
    headers: HeaderMap,
    axum::Extension(kind): axum::Extension<ListenerKind>,
    Json(b): Json<LoginReq>,
) -> ApiResult<Response> {
    if !st.rate.check(&rate_key(&peer, &headers, "login"), 10, Duration::from_secs(300)) {
        return Err(ApiError::too_many());
    }
    let ip = auth::client_ip(Some(peer.0), &headers).map(|i| i.to_string());
    let found = auth::find_user(&st, b.username.trim()).await?;
    let user = match found {
        Some((u, hash, disabled)) if !disabled && auth::verify_password(&b.password, &hash) => u,
        Some(_) => {
            audit::record(&st.db, b.username.clone(), "auth.login.failed", None, None, ip).await;
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "sai tên đăng nhập hoặc mật khẩu"));
        }
        None => {
            auth::dummy_verify(&b.password);
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_credentials", "sai tên đăng nhập hoặc mật khẩu"));
        }
    };
    let ttl = if user.role == "owner" { auth::OWNER_TTL } else { auth::VIEWER_TTL };
    let device = b.device_name.unwrap_or_else(|| "Trình duyệt".into());
    let s = auth::create_session(&st, &user.id, &device, ua(&headers), ip.clone(), false, ttl).await?;
    audit::record(&st.db, user.username.clone(), "auth.login", None, Some(json!({ "device": device })), ip).await;
    let a = Auth { user_id: user.id, username: user.username, role: user.role, host_control: false, session_id: s.id, csrf: s.csrf, ip: None };
    let mut body = me_json(&st, Some(&a)).await;
    if b.auth_mode.as_deref() == Some("bearer") {
        body["sessionToken"] = json!(s.token);
        return Ok(Json(body).into_response());
    }
    let secure = auth::is_https(Some(peer.0), &headers);
    Ok(with_cookie(auth::session_cookie(kind, &s.token, ttl, secure), body))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairReq {
    code: String,
    device_name: Option<String>,
    auth_mode: Option<String>,
}

async fn pair(
    State(st): St,
    peer: Peer,
    headers: HeaderMap,
    axum::Extension(kind): axum::Extension<ListenerKind>,
    Json(b): Json<PairReq>,
) -> ApiResult<Response> {
    if !st.rate.check(&rate_key(&peer, &headers, "pair"), 10, Duration::from_secs(300)) {
        return Err(ApiError::too_many());
    }
    let code_hash = doto_core::sha256_hex(auth::normalize_code(&b.code));
    let device: String =
        b.device_name.clone().unwrap_or_else(|| "Thiết bị".into()).chars().filter(|c| c.is_alphanumeric() || " -_".contains(*c)).take(40).collect();
    let now = doto_core::now_rfc3339();
    let uname = format!("device-{}", &doto_core::new_id()[..8]);
    let (ch, n2, un2) = (code_hash.clone(), now.clone(), uname.clone());
    let user: Option<(String, String)> = st
        .db
        .call(move |c| {
            let tx = c.transaction()?;
            let role: Option<String> = tx
                .query_row("SELECT role FROM pairing_codes WHERE code_hash=?1 AND used_at IS NULL AND expires_at > ?2", params![ch, n2], |r| r.get(0))
                .optional()?;
            let Some(role) = role else { return Ok(None) };
            let uid = doto_core::new_id();
            tx.execute("INSERT INTO users(id,username,password_hash,role,created_at) VALUES(?1,?2,'!',?3,?4)", params![uid, un2, role, n2])?;
            tx.execute("UPDATE pairing_codes SET used_at=?2, used_by_user=?3 WHERE code_hash=?1", params![ch, n2, uid])?;
            tx.commit()?;
            Ok(Some((uid, role)))
        })
        .await?;
    let Some((uid, role)) = user else {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid_code", "mã ghép nối sai, đã dùng hoặc đã hết hạn"));
    };
    let ip = auth::client_ip(Some(peer.0), &headers).map(|i| i.to_string());
    let ttl = if role == "owner" { auth::OWNER_TTL } else { auth::VIEWER_TTL };
    let s = auth::create_session(&st, &uid, &device, ua(&headers), ip.clone(), false, ttl).await?;
    audit::record(&st.db, uname.clone(), "auth.pair", None, Some(json!({ "device": device, "role": role })), ip).await;
    let a = Auth { user_id: uid, username: uname, role, host_control: false, session_id: s.id, csrf: s.csrf, ip: None };
    let mut body = me_json(&st, Some(&a)).await;
    if b.auth_mode.as_deref() == Some("bearer") {
        body["sessionToken"] = json!(s.token);
        return Ok(Json(body).into_response());
    }
    Ok(with_cookie(auth::session_cookie(kind, &s.token, ttl, auth::is_https(Some(peer.0), &headers)), body))
}

async fn issue_ticket(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    let ticket = st.issue_media_ticket(&a.session_id);
    Ok(Json(json!({ "ticket": ticket, "expiresIn": 600 })))
}

async fn logout(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Response> {
    let sid = a.session_id.clone();
    st.db
        .call(move |c| {
            c.execute("UPDATE sessions SET revoked=1 WHERE id=?1", [sid])?;
            Ok(())
        })
        .await?;
    audit_ev(&st, &a, "auth.logout", None, None).await;
    Ok(with_cookie(auth::clear_cookie(kind), json!({ "ok": true })))
}

// ======================= Users / devices / pairing =======================

async fn list_users(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v: Vec<auth::UserDto> = st
        .db
        .call(|c| {
            let mut s = c.prepare("SELECT id, username, role, created_at FROM users WHERE disabled=0 ORDER BY created_at")?;
            let v = s
                .query_map([], |r| Ok(auth::UserDto { id: r.get(0)?, username: r.get(1)?, role: r.get(2)?, created_at: r.get(3)? }))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(v)))
}

#[derive(Deserialize)]
struct CreateUserReq {
    username: String,
    password: String,
    role: String,
}

async fn create_user(State(st): St, a: Auth, Json(b): Json<CreateUserReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    auth::username_policy(&b.username).map_err(ApiError::validation)?;
    auth::password_policy(&b.password).map_err(ApiError::validation)?;
    if !matches!(b.role.as_str(), "owner" | "viewer") {
        return Err(ApiError::validation("role phải là owner hoặc viewer"));
    }
    let hash = auth::hash_password(&b.password)?;
    let id = doto_core::new_id();
    let (i, u, r, now) = (id.clone(), b.username.clone(), b.role.clone(), doto_core::now_rfc3339());
    let n2 = now.clone();
    st.db
        .call(move |c| {
            c.execute("INSERT INTO users(id,username,password_hash,role,created_at) VALUES(?1,?2,?3,?4,?5)", params![i, u, hash, r, n2])?;
            Ok(())
        })
        .await
        .map_err(|e| if e.to_string().contains("UNIQUE") { ApiError::conflict("tên đăng nhập đã tồn tại") } else { ApiError::internal(e) })?;
    audit_ev(&st, &a, "user.create", Some(b.username.clone()), Some(json!({ "role": b.role }))).await;
    Ok(Json(json!({ "id": id, "username": b.username, "role": b.role, "createdAt": now })))
}

async fn delete_user(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if id == a.user_id {
        return Err(ApiError::validation("không thể xoá chính mình"));
    }
    let i = id.clone();
    let n = st
        .db
        .call(move |c| {
            let role: Option<String> = c.query_row("SELECT role FROM users WHERE id=?1", [&i], |r| r.get(0)).optional()?;
            if role.as_deref() == Some("owner") {
                let owners: i64 = c.query_row("SELECT count(*) FROM users WHERE role='owner' AND disabled=0", [], |r| r.get(0))?;
                anyhow::ensure!(owners > 1, "LAST_OWNER");
            }
            Ok(c.execute("DELETE FROM users WHERE id=?1", [&i])?)
        })
        .await
        .map_err(|e| if e.to_string() == "LAST_OWNER" { ApiError::validation("không thể xoá Owner cuối cùng") } else { ApiError::internal(e) })?;
    if n == 0 {
        return Err(ApiError::not_found("không tìm thấy người dùng"));
    }
    audit_ev(&st, &a, "user.delete", Some(id), None).await;
    Ok(ok())
}

#[derive(Deserialize)]
struct PasswordReq {
    password: String,
}

async fn set_user_password(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<PasswordReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    auth::password_policy(&b.password).map_err(ApiError::validation)?;
    let hash = auth::hash_password(&b.password)?;
    let i = id.clone();
    let n = st
        .db
        .call(move |c| {
            let n = c.execute("UPDATE users SET password_hash=?2 WHERE id=?1", params![i, hash])?;
            // đổi mật khẩu → thu hồi phiên cũ (trừ phiên desktop)
            c.execute("UPDATE sessions SET revoked=1 WHERE user_id=?1 AND host_control=0", [&i])?;
            Ok(n)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::not_found("không tìm thấy người dùng"));
    }
    audit_ev(&st, &a, "user.password", Some(id), None).await;
    Ok(ok())
}

async fn list_devices(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cur = a.session_id.clone();
    let now = doto_core::now_rfc3339();
    let v: Vec<Value> = st
        .db
        .call(move |c| {
            let mut s = c.prepare(
                "SELECT s.id, u.id, u.username, u.role, s.device_name, s.user_agent, s.ip, s.created_at, s.last_seen_at, s.expires_at, s.host_control
                 FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.revoked=0 AND s.expires_at > ?1 ORDER BY s.last_seen_at DESC",
            )?;
            let v = s
                .query_map([now], |r| {
                    let id: String = r.get(0)?;
                    Ok(json!({
                        "id": id, "userId": r.get::<_, String>(1)?, "username": r.get::<_, String>(2)?, "role": r.get::<_, String>(3)?,
                        "deviceName": r.get::<_, String>(4)?, "userAgent": r.get::<_, Option<String>>(5)?, "ip": r.get::<_, Option<String>>(6)?,
                        "createdAt": r.get::<_, String>(7)?, "lastSeenAt": r.get::<_, String>(8)?, "expiresAt": r.get::<_, String>(9)?,
                        "hostControl": r.get::<_, i64>(10)? != 0, "current": id == cur,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(v)))
}

async fn revoke_device(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let i = id.clone();
    let n = st.db.call(move |c| Ok(c.execute("UPDATE sessions SET revoked=1 WHERE id=?1", [i])?)).await?;
    if n == 0 {
        return Err(ApiError::not_found("không tìm thấy phiên"));
    }
    audit_ev(&st, &a, "device.revoke", Some(id), None).await;
    Ok(ok())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingReq {
    role: Option<String>,
    ttl_minutes: Option<i64>,
}

fn qr_svg(data: &str) -> Option<String> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    Some(code.render::<qrcode::render::svg::Color>().min_dimensions(220, 220).quiet_zone(true).build())
}

async fn create_pairing(State(st): St, a: Auth, Json(b): Json<PairingReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let role = b.role.unwrap_or_else(|| "viewer".into());
    if !matches!(role.as_str(), "owner" | "viewer") {
        return Err(ApiError::validation("role không hợp lệ"));
    }
    let ttl = b.ttl_minutes.unwrap_or(15).clamp(5, 1440);
    let code = auth::new_pairing_code();
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(ttl)).to_rfc3339();
    let (h, r, by, e) = (doto_core::sha256_hex(&code), role.clone(), a.user_id.clone(), expires.clone());
    st.db
        .call(move |c| {
            c.execute(
                "INSERT INTO pairing_codes(code_hash,role,created_by,created_at,expires_at) VALUES(?1,?2,?3,?4,?5)",
                params![h, r, by, doto_core::now_rfc3339(), e],
            )?;
            Ok(())
        })
        .await?;
    let cfg = st.server_config().await;
    let status = st.public.status(&cfg);
    let base = cfg.public_base_url.clone().or_else(|| status.urls.iter().find(|u| !u.contains("127.0.0.1")).cloned()).or_else(|| status.urls.first().cloned());
    let url = base.map(|b| format!("{}/?pair={code}", b.trim_end_matches('/')));
    audit_ev(&st, &a, "pairing.create", None, Some(json!({ "role": role, "ttlMinutes": ttl }))).await;
    Ok(Json(json!({ "code": code, "expiresAt": expires, "role": role, "qrSvg": url.as_deref().and_then(qr_svg), "url": url })))
}

// ======================= System / settings =======================

async fn get_hardware(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let hw = st.hardware.read().unwrap().clone();
    match hw {
        Some(h) => Ok(Json(json!(h))),
        None => Ok(Json(json!(st.probe_hardware().await?))),
    }
}

async fn probe_hardware(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.probe_hardware().await?)))
}

async fn get_metrics(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(st.metrics()))
}

async fn get_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.media.status())))
}

async fn probe_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.media.refresh_ffmpeg().await;
    if !s.found {
        return Ok(Json(json!(s)));
    }
    Ok(Json(json!(st.media.probe_encoders().await?)))
}

async fn benchmark_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let results = st.media.benchmark_encoders().await?;
    audit_ev(&st, &a, "system.ffmpeg.benchmark", None, Some(json!({ "encoders": results.len() }))).await;
    Ok(Json(json!({ "results": results, "ranAt": doto_core::now_rfc3339() })))
}

async fn list_backups(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let dir = st.paths.backups_dir();
    let list = tokio::task::spawn_blocking(move || -> Vec<Value> {
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        let mut items: Vec<(String, u64, String)> = rd
            .filter_map(Result::ok)
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                if !meta.is_file() {
                    return None;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".sqlite") {
                    return None;
                }
                let mtime = meta.modified().ok().map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()).unwrap_or_default();
                Some((name, meta.len(), mtime))
            })
            .collect();
        items.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.0.cmp(&a.0)));
        items.into_iter().map(|(file, size, created_at)| json!({ "file": file, "sizeBytes": size, "createdAt": created_at })).collect()
    })
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(json!(list)))
}

async fn backup(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let name = format!("doto-{}.sqlite", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let dest = st.paths.backups_dir().join(&name);
    st.db.backup_to(dest).await?;
    audit_ev(&st, &a, "system.backup", Some(name.clone()), None).await;
    Ok(Json(json!({ "ok": true, "file": name })))
}

#[derive(Deserialize)]
struct RestoreReq {
    file: String,
    confirm: bool,
}

async fn restore_backup(State(st): St, a: Auth, Json(b): Json<RestoreReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi khôi phục cơ sở dữ liệu"));
    }
    let name = b.file.trim();
    let valid_name = !name.is_empty()
        && name.ends_with(".sqlite")
        && !name.contains("..")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid_name {
        return Err(ApiError::validation("tên bản sao lưu không hợp lệ"));
    }
    let src = st.paths.backups_dir().join(name);
    if !src.is_file() {
        return Err(ApiError::not_found("không tìm thấy bản sao lưu"));
    }
    st.db.restore_from(src).await.map_err(|e| ApiError::validation(e.to_string()))?;
    let stored: doto_system::StoredResourceSettings = settings::get(&st.db, settings::KEY_RESOURCES).await.unwrap_or_default();
    st.rm.apply(stored);
    st.media.refresh_ffmpeg().await;
    st.skills.restore_watchers().await;
    audit_ev(&st, &a, "system.restore", Some(name.to_string()), None).await;
    Ok(Json(json!({ "ok": true, "file": name })))
}

async fn get_resources(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.rm.settings();
    Ok(Json(json!({ "profile": s.profile, "overrides": s.overrides, "effective": st.rm.limits() })))
}

async fn put_resources(State(st): St, a: Auth, Json(b): Json<doto_system::StoredResourceSettings>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    settings::set(&st.db, settings::KEY_RESOURCES, &b).await?;
    let limits = st.rm.apply(b.clone());
    st.media.wake();
    audit_ev(&st, &a, "resources.update", None, Some(json!(b))).await;
    Ok(Json(json!({ "profile": b.profile, "overrides": b.overrides, "effective": limits })))
}

async fn settings_json(st: &AppState) -> ApiResult<Value> {
    let s: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await?;
    let mut v = json!(s);
    v["dataDir"] = json!(st.paths.data_dir.to_string_lossy());
    v["defaultUploadsDir"] = json!(st.paths.default_uploads_dir().to_string_lossy());
    v["defaultCustomSkillsDir"] = json!(st.paths.default_custom_skills_dir().to_string_lossy());
    Ok(v)
}

async fn get_settings(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(settings_json(&st).await?))
}

fn validate_binary_name(path_str: &str, expected_prefix: &str) -> Result<(), String> {
    let p = std::path::Path::new(path_str);
    if !p.is_absolute() || !p.is_file() {
        return Err(format!("đường dẫn {expected_prefix} phải là tệp tuyệt đối tồn tại trên máy chủ"));
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !stem.starts_with(expected_prefix) {
        return Err(format!("tên tệp thực thi phải bắt đầu bằng `{expected_prefix}`"));
    }
    Ok(())
}

async fn put_settings(State(st): St, a: Auth, Json(b): Json<settings::AppSettings>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    b.validate().map_err(ApiError::validation)?;
    for d in &b.video_source_dirs {
        if !std::path::Path::new(d).is_dir() {
            return Err(ApiError::validation(format!("thư mục không tồn tại: {d}")));
        }
    }
    if let Some(fp) = &b.ffmpeg_path {
        validate_binary_name(fp, "ffmpeg").map_err(ApiError::validation)?;
    }
    if let Some(pp) = &b.ffprobe_path {
        validate_binary_name(pp, "ffprobe").map_err(ApiError::validation)?;
    }
    if b.ffmpeg_path.is_some() || b.ffprobe_path.is_some() {
        doto_media::ffmpeg::discover(b.ffmpeg_path.as_deref(), b.ffprobe_path.as_deref())
            .await
            .map_err(|e| ApiError::validation(format!("xác minh FFmpeg/ffprobe thất bại: {e}")))?;
    }
    let old: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await?;
    settings::set(&st.db, settings::KEY_APP, &b).await?;
    if old.ffmpeg_path != b.ffmpeg_path || old.ffprobe_path != b.ffprobe_path {
        st.media.refresh_ffmpeg().await;
    }
    audit_ev(&st, &a, "settings.update", None, Some(json!({ "videoSourceDirs": b.video_source_dirs.len(), "outputDir": b.output_dir.is_some() }))).await;
    Ok(Json(settings_json(&st).await?))
}

async fn get_onboarding(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let o: settings::OnboardingState = settings::get(&st.db, settings::KEY_ONBOARDING).await?;
    Ok(Json(json!(o)))
}

async fn complete_onboarding(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let now = doto_core::now_rfc3339();
    let o = settings::OnboardingState { completed: true, completed_at: Some(now.clone()), accepted_terms_at: Some(now) };
    settings::set(&st.db, settings::KEY_ONBOARDING, &o).await?;
    audit_ev(&st, &a, "onboarding.complete", None, None).await;
    Ok(Json(json!(o)))
}

async fn events(State(st): St, a: Auth) -> Sse<impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    let rx = st.events.subscribe();
    let owner = a.is_owner();
    let stream = futures::stream::unfold(rx, move |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if !owner && !ev.viewer_visible {
                        continue;
                    }
                    let data = serde_json::to_string(&ev).unwrap_or_default();
                    return Some((Ok(SseEvent::default().data(data)), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[derive(Deserialize)]
struct LimitQ {
    limit: Option<u32>,
}

async fn list_audit(State(st): St, a: Auth, Query(q): Query<LimitQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(audit::list(&st.db, q.limit.unwrap_or(200)).await?)))
}

// ======================= Skills =======================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillQ {
    q: Option<String>,
    source_id: Option<String>,
}

async fn list_skills(State(st): St, a: Auth, Query(q): Query<SkillQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.list_skills(q.q, q.source_id).await?)))
}

async fn skill_detail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.detail(&id).await?)))
}

async fn list_sources(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.list_sources().await?)))
}

#[derive(Deserialize)]
struct FolderReq {
    path: String,
    watch: Option<bool>,
}

async fn add_folder_source(State(st): St, a: Auth, Json(b): Json<FolderReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.add_folder(&b.path, b.watch.unwrap_or(false)).await?;
    audit_ev(&st, &a, "skills.source.add_folder", Some(s.id.clone()), Some(json!({ "skills": s.skill_count }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubReq {
    url: String,
    #[serde(rename = "ref")]
    git_ref: Option<String>,
}

async fn add_github_source(State(st): St, a: Auth, Json(b): Json<GithubReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.add_github(&b.url, b.git_ref.as_deref()).await?;
    audit_ev(&st, &a, "skills.source.add_github", Some(s.id.clone()), Some(json!({ "repo": s.location, "sha": s.current_sha, "private": s.private }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
struct SourcePatch {
    enabled: Option<bool>,
    watch: Option<bool>,
    pinned: Option<bool>,
}

async fn patch_source(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<SourcePatch>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.set_flags(&id, b.enabled, b.watch, b.pinned).await?;
    audit_ev(&st, &a, "skills.source.update", Some(id), Some(json!({ "enabled": b.enabled, "watch": b.watch, "pinned": b.pinned }))).await;
    Ok(Json(json!(s)))
}

async fn delete_source(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.skills.remove(&id).await?;
    audit_ev(&st, &a, "skills.source.remove", Some(id), None).await;
    Ok(ok())
}

async fn rescan_source(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.rescan(&id).await?)))
}

async fn check_update(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.skills.check_update(&id).await?)))
}

#[derive(Deserialize)]
struct ApplyReq {
    sha: String,
}

async fn apply_update(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<ApplyReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.skills.apply_update(&id, &b.sha).await?;
    audit_ev(&st, &a, "skills.source.apply_update", Some(id), Some(json!({ "sha": b.sha }))).await;
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallSkillReq {
    provider: String,
    confirm: bool,
    allow_scripts: Option<bool>,
}

async fn install_skill(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<InstallSkillReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let target = st.skills.install(&id, &b.provider, b.confirm, b.allow_scripts.unwrap_or(false)).await?;
    audit_ev(&st, &a, "skills.install", Some(id), Some(json!({ "provider": b.provider, "allowScripts": b.allow_scripts }))).await;
    Ok(Json(json!({ "ok": true, "target": target })))
}

async fn uninstall_skill(State(st): St, a: Auth, Path((id, provider)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.skills.uninstall(&id, &provider).await?;
    audit_ev(&st, &a, "skills.uninstall", Some(id), Some(json!({ "provider": provider }))).await;
    Ok(ok())
}

// ======================= GitHub =======================

async fn github_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn github_device_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn github_device_poll(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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
struct ConfirmReq {
    confirm: bool,
}

async fn github_use_gh(State(st): St, a: Auth, Json(b): Json<ConfirmReq>) -> ApiResult<Json<Value>> {
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
    let names = if cfg!(windows) { vec!["gh.exe"] } else { vec!["gh"] };
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|c| c.is_file())).or_else(|| {
        let cands = if cfg!(windows) { vec![r"C:\Program Files\GitHub CLI\gh.exe"] } else { vec!["/opt/homebrew/bin/gh", "/usr/local/bin/gh"] };
        cands.into_iter().map(std::path::PathBuf::from).find(|p| p.is_file())
    })
}

async fn github_logout(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn list_tools(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn refresh_tool(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.tools.status(&id).await.ok_or_else(|| ApiError::not_found("công cụ không tồn tại"))?;
    st.events.emit("tool", &s);
    Ok(Json(json!(s)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallToolReq {
    method_id: String,
    confirm: bool,
}

async fn install_tool(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<InstallToolReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi cài phần mềm"));
    }
    let job = st.tools.install(&id, &b.method_id).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tools.install", Some(id), Some(json!({ "method": b.method_id, "jobId": job.id }))).await;
    Ok(Json(json!(job)))
}

async fn tool_diagnostics(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let lines = st.tools.diagnostics(&id).await.ok_or_else(|| ApiError::not_found("công cụ không tồn tại"))?;
    Ok(Json(json!({ "lines": lines })))
}

// ======================= Server / tunnel =======================

async fn server_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = st.server_config().await;
    Ok(Json(json!(st.public.status(&cfg))))
}

async fn get_server_config(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.server_config().await)))
}

async fn put_server_config(State(st): St, a: Auth, Json(b): Json<ServerConfig>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    b.validate().map_err(ApiError::validation)?;
    if b.mode == "lan" && !auth::owner_password_set(&st).await {
        return Err(ApiError::validation("cần đặt mật khẩu Owner trước khi chọn chế độ LAN"));
    }
    settings::set(&st.db, settings::KEY_SERVER, &b).await?;
    audit_ev(&st, &a, "server.config", None, Some(json!({ "mode": b.mode, "port": b.port, "allowlist": b.ip_allowlist.len() }))).await;
    Ok(Json(json!(b)))
}

async fn server_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.start_public().await.map_err(ApiError::conflict)?;
    audit_ev(&st, &a, "server.start", None, None).await;
    let cfg = st.server_config().await;
    Ok(Json(json!(st.public.status(&cfg))))
}

async fn server_stop(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    audit_ev(&st, &a, "server.stop", None, None).await;
    let cfg = st.server_config().await;
    if kind == ListenerKind::Public {
        // Tránh tự khoá (self-deadlock) khi request đang chạy bên trong chính listener Public.
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

async fn server_restart(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Json<Value>> {
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
struct QrQ {
    url: String,
}

async fn server_qr(a: Auth, Query(q): Query<QrQ>) -> ApiResult<Response> {
    a.require_owner()?;
    if q.url.len() > 512 || !(q.url.starts_with("http://") || q.url.starts_with("https://")) {
        return Err(ApiError::validation("URL không hợp lệ"));
    }
    let svg = qr_svg(&q.url).ok_or_else(|| ApiError::validation("không tạo được QR"))?;
    Ok(([(header::CONTENT_TYPE, "image/svg+xml"), (header::CACHE_CONTROL, "no-store")], svg).into_response())
}

async fn firewall_guide(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(public::firewall_guide(st.server_config().await.port)))
}

async fn tunnel_cfg(st: &AppState) -> doto_integrations::TunnelConfig {
    settings::get(&st.db, settings::KEY_TUNNEL).await.unwrap_or_default()
}

async fn tunnel_status(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let cfg = tunnel_cfg(&st).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunnelReq {
    mode: String,
    hostname: Option<String>,
    auto_start: Option<bool>,
    token: Option<String>,
}

async fn put_tunnel_config(State(st): St, a: Auth, Json(b): Json<TunnelReq>) -> ApiResult<Json<Value>> {
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

async fn tunnel_start(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn tunnel_stop(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.stop().await;
    audit_ev(&st, &a, "tunnel.stop", None, None).await;
    let cfg = tunnel_cfg(&st).await;
    Ok(Json(json!(st.tunnel.status(&cfg, st.cloudflared_path()))))
}

async fn delete_tunnel_token(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.delete_token().map_err(ApiError::internal)?;
    audit_ev(&st, &a, "tunnel.token.delete", None, None).await;
    Ok(ok())
}

// ======================= Video =======================

#[derive(Deserialize)]
struct VideoQ {
    kind: Option<String>,
    q: Option<String>,
}

fn can_view(a: &Auth, v: &library::VideoAsset) -> bool {
    a.is_owner() || (v.kind == "export" && v.visibility == "viewers" && v.status == "ready")
}

/// Lấy video + kiểm tra quyền; không có quyền → 404 (không tiết lộ tồn tại, chống IDOR).
async fn visible_video(st: &AppState, a: &Auth, id: &str) -> ApiResult<library::VideoAsset> {
    let v = library::get(&st.db, id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy video"))?;
    if !can_view(a, &v) {
        return Err(ApiError::not_found("không tìm thấy video"));
    }
    Ok(v)
}

async fn list_videos(State(st): St, a: Auth, Query(q): Query<VideoQ>) -> ApiResult<Json<Value>> {
    let v = library::list(&st.db, library::ListFilter { kind: q.kind, q: q.q, viewer_only: !a.is_owner() }).await?;
    Ok(Json(json!(v)))
}

async fn get_video(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(visible_video(&st, &a, &id).await?)))
}

#[derive(Deserialize)]
struct VideoPatch {
    title: Option<String>,
    visibility: Option<String>,
}

async fn patch_video(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<VideoPatch>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    if b.visibility.as_deref() == Some("viewers") && v.kind != "export" {
        return Err(ApiError::validation("chỉ video đã xuất mới chia sẻ cho Viewer"));
    }
    if !library::update_meta(&st.db, &id, b.title, b.visibility.clone()).await.map_err(|e| ApiError::validation(e.to_string()))? {
        return Err(ApiError::not_found("không tìm thấy video"));
    }
    let v = visible_video(&st, &a, &id).await?;
    st.events.emit_scoped("video", &v, v.visibility == "viewers");
    audit_ev(&st, &a, "video.update", Some(id), Some(json!({ "visibility": b.visibility }))).await;
    Ok(Json(json!(v)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteQ {
    delete_file: Option<bool>,
}

async fn delete_video(State(st): St, a: Auth, Path(id): Path<String>, Query(q): Query<DeleteQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    let delete_file = q.delete_file.unwrap_or(false);
    if delete_file && v.kind != "export" {
        return Err(ApiError::validation("không xoá file video nguồn của người dùng — chỉ gỡ khỏi thư viện"));
    }
    library::delete(&st.db, &id).await?;
    if let Some(t) = &v.thumbnail_path {
        let _ = tokio::fs::remove_file(t).await;
    }
    if delete_file {
        let _ = tokio::fs::remove_file(&v.file_path).await;
    }
    audit_ev(&st, &a, "video.delete", Some(id), Some(json!({ "deleteFile": delete_file, "kind": v.kind }))).await;
    Ok(ok())
}

async fn video_thumbnail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let p = v.thumbnail_path.filter(|p| p.is_file()).ok_or_else(|| ApiError::not_found("chưa có thumbnail"))?;
    let bytes = tokio::fs::read(p).await.map_err(ApiError::internal)?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=3600")], bytes).into_response())
}

async fn stream_permit(st: &AppState) -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    let total = 24usize;
    let in_use = total - st.stream_slots.available_permits();
    if in_use >= st.stream_limit() {
        return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "busy", "máy chủ đang phát nhiều luồng — thử lại sau"));
    }
    st.stream_slots.clone().try_acquire_owned().map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "busy", "máy chủ đang bận"))
}

async fn bytes_per_sec(st: &AppState) -> Option<u64> {
    let kbps = st.server_config().await.max_stream_kbps;
    (kbps > 0).then_some(kbps * 1000 / 8)
}

async fn video_stream(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let permit = stream_permit(&st).await?;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: None, bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) }).await
}

async fn video_download(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let permit = stream_permit(&st).await?;
    let ext = v.file_path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "mp4".into());
    let name = format!("{}.{}", doto_media::library::sanitize_file_stem(&v.title), ext);
    audit_ev(&st, &a, "video.download", Some(id), None).await;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: Some(&name), bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) })
        .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShareReq {
    ttl_minutes: Option<i64>,
}

async fn create_share(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<ShareReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    if v.kind != "export" {
        return Err(ApiError::validation("chỉ chia sẻ được video đã xuất"));
    }
    let ttl = b.ttl_minutes.unwrap_or(60).clamp(5, 7 * 24 * 60);
    let token = doto_core::random_token(24);
    let sid = doto_core::new_id();
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(ttl)).to_rfc3339();
    let (s2, th, vid, by, e2) = (sid.clone(), doto_core::sha256_hex(&token), id.clone(), a.user_id.clone(), expires.clone());
    st.db
        .call(move |c| {
            c.execute(
                "INSERT INTO share_links(id,token_hash,video_id,created_by,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![s2, th, vid, by, doto_core::now_rfc3339(), e2],
            )?;
            Ok(())
        })
        .await?;
    let cfg = st.server_config().await;
    let base = cfg.public_base_url.clone().or_else(|| st.public.status(&cfg).urls.into_iter().last()).unwrap_or_default();
    audit_ev(&st, &a, "share.create", Some(id.clone()), Some(json!({ "ttlMinutes": ttl }))).await;
    Ok(Json(json!({ "id": sid, "videoId": id, "url": format!("{}/s/{token}", base.trim_end_matches('/')), "expiresAt": expires })))
}

async fn list_shares(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let _ = visible_video(&st, &a, &id).await?;
    let now = doto_core::now_rfc3339();
    let vid = id.clone();
    let list: Vec<Value> = st
        .db
        .call(move |c| {
            let mut s = c.prepare(
                "SELECT id, video_id, created_at, expires_at FROM share_links WHERE video_id=?1 AND revoked=0 AND expires_at > ?2 ORDER BY created_at DESC",
            )?;
            let v = s
                .query_map(params![vid, now], |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?,
                        "videoId": r.get::<_, String>(1)?,
                        "url": null,
                        "createdAt": r.get::<_, String>(2)?,
                        "expiresAt": r.get::<_, String>(3)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(list)))
}

async fn revoke_share(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let i = id.clone();
    let n = st.db.call(move |c| Ok(c.execute("UPDATE share_links SET revoked=1 WHERE id=?1", [i])?)).await?;
    if n == 0 {
        return Err(ApiError::not_found("không tìm thấy link"));
    }
    audit_ev(&st, &a, "share.revoke", Some(id), None).await;
    Ok(ok())
}

async fn share_stream(State(st): St, Path(token): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let th = doto_core::sha256_hex(&token);
    let now = doto_core::now_rfc3339();
    let vid: Option<String> = st
        .db
        .call(move |c| {
            Ok(c.query_row("SELECT video_id FROM share_links WHERE token_hash=?1 AND revoked=0 AND expires_at > ?2", params![th, now], |r| r.get(0))
                .optional()?)
        })
        .await?;
    let vid = vid.ok_or_else(|| ApiError::not_found("link không tồn tại hoặc đã hết hạn"))?;
    let v = library::get(&st.db, &vid).await?.filter(|v| v.status == "ready").ok_or_else(|| ApiError::not_found("video không còn"))?;
    let permit = stream_permit(&st).await?;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: None, bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) }).await
}

async fn scan_videos(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.media.scan_sources().await?;
    audit_ev(&st, &a, "video.scan", None, Some(json!({ "added": s.added_or_updated, "missing": s.missing }))).await;
    Ok(Json(json!(s)))
}

// ======================= Jobs =======================

async fn list_jobs(State(st): St, a: Auth, Query(q): Query<LimitQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(jobs::list(&st.db, q.limit.unwrap_or(200)).await?)))
}

async fn job_detail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(jobs::detail(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?)))
}

async fn create_render(State(st): St, a: Auth, Json(b): Json<doto_media::RenderRequest>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let job = st.media.submit_render(b).await?;
    audit_ev(&st, &a, "job.render", Some(job.id.clone()), None).await;
    Ok(Json(json!(job)))
}

async fn cancel_job(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let j = jobs::get(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?;
    if j.kind == "studio" {
        let n = st.studio.cancel_job(&id).await?;
        if n == 0 && !matches!(j.status.as_str(), "running" | "queued") {
            return Err(ApiError::conflict("job đã kết thúc"));
        }
        audit_ev(&st, &a, "job.cancel", Some(id), Some(json!({ "kind": "studio" }))).await;
        return Ok(ok());
    }
    if j.kind != "render" {
        return Err(ApiError::conflict("chỉ huỷ được job render hoặc AI Studio"));
    }
    if !st.media.cancel(&id).await? {
        return Err(ApiError::conflict("job đã kết thúc"));
    }
    audit_ev(&st, &a, "job.cancel", Some(id), None).await;
    Ok(ok())
}

async fn retry_job(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let j = jobs::get(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?;
    if j.kind != "render" {
        return Err(ApiError::conflict("chỉ thử lại được job render"));
    }
    if !st.media.retry(&id).await? {
        return Err(ApiError::conflict("chỉ thử lại job thất bại hoặc đã huỷ"));
    }
    audit_ev(&st, &a, "job.retry", Some(id.clone()), None).await;
    Ok(Json(json!(jobs::get(&st.db, &id).await?)))
}

#[allow(dead_code)]
fn _unused(_: HashMap<(), ()>) {}

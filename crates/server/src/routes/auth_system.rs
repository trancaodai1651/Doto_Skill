//! Route xác thực, người dùng, thiết bị, mã ghép nối, phần cứng, tài nguyên, cài đặt,
//! sao lưu/khôi phục SQLite, SSE và nhật ký kiểm toán.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_core::{audit, settings};

use crate::auth::{self, Auth, ListenerKind, MaybeAuth};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

use super::{audit_ev, ok, qr_svg, rate_key, ua, LimitQ, Peer, St};

// ======================= Auth =======================

pub(crate) async fn health(State(st): St) -> Json<Value> {
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

pub(crate) async fn me(State(st): St, MaybeAuth(a): MaybeAuth) -> Json<Value> {
    Json(me_json(&st, a.as_ref()).await)
}

fn with_cookie(cookie: String, body: Value) -> Response {
    ([(header::SET_COOKIE, cookie)], Json(body)).into_response()
}

#[derive(Deserialize)]
pub(crate) struct DesktopReq {
    token: String,
}

pub(crate) async fn desktop_login(State(st): St, peer: Peer, headers: HeaderMap, Json(b): Json<DesktopReq>) -> ApiResult<Response> {
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
pub(crate) struct SetupReq {
    username: String,
    password: String,
    auth_mode: Option<String>,
}

pub(crate) async fn setup_owner(
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
pub(crate) struct LoginReq {
    username: String,
    password: String,
    device_name: Option<String>,
    auth_mode: Option<String>,
}

pub(crate) async fn login(
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
pub(crate) struct PairReq {
    code: String,
    device_name: Option<String>,
    auth_mode: Option<String>,
}

pub(crate) async fn pair(
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

pub(crate) async fn issue_ticket(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    let ticket = st.issue_media_ticket(&a.session_id);
    Ok(Json(json!({ "ticket": ticket, "expiresIn": 600 })))
}

pub(crate) async fn logout(State(st): St, a: Auth, axum::Extension(kind): axum::Extension<ListenerKind>) -> ApiResult<Response> {
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

pub(crate) async fn list_users(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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
pub(crate) struct CreateUserReq {
    username: String,
    password: String,
    role: String,
}

pub(crate) async fn create_user(State(st): St, a: Auth, Json(b): Json<CreateUserReq>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn delete_user(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
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
pub(crate) struct PasswordReq {
    password: String,
}

pub(crate) async fn set_user_password(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<PasswordReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    auth::password_policy(&b.password).map_err(ApiError::validation)?;
    let hash = auth::hash_password(&b.password)?;
    let i = id.clone();
    let n = st
        .db
        .call(move |c| {
            let n = c.execute("UPDATE users SET password_hash=?2 WHERE id=?1", params![i, hash])?;
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

pub(crate) async fn list_devices(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

pub(crate) async fn revoke_device(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
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
pub(crate) struct PairingReq {
    role: Option<String>,
    ttl_minutes: Option<i64>,
}

pub(crate) async fn create_pairing(State(st): St, a: Auth, Json(b): Json<PairingReq>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn get_hardware(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let hw = st.hardware.read().unwrap().clone();
    match hw {
        Some(h) => Ok(Json(json!(h))),
        None => Ok(Json(json!(st.probe_hardware().await?))),
    }
}

pub(crate) async fn probe_hardware(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.probe_hardware().await?)))
}

pub(crate) async fn get_metrics(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(st.metrics()))
}

pub(crate) async fn get_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.media.status())))
}

pub(crate) async fn probe_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.media.refresh_ffmpeg().await;
    if !s.found {
        return Ok(Json(json!(s)));
    }
    Ok(Json(json!(st.media.probe_encoders().await?)))
}

pub(crate) async fn benchmark_ffmpeg(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let results = st.media.benchmark_encoders().await?;
    audit_ev(&st, &a, "system.ffmpeg.benchmark", None, Some(json!({ "encoders": results.len() }))).await;
    Ok(Json(json!({ "results": results, "ranAt": doto_core::now_rfc3339() })))
}

pub(crate) async fn list_backups(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

pub(crate) async fn backup(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let name = format!("doto-{}.sqlite", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let dest = st.paths.backups_dir().join(&name);
    st.db.backup_to(dest).await?;
    audit_ev(&st, &a, "system.backup", Some(name.clone()), None).await;
    Ok(Json(json!({ "ok": true, "file": name })))
}

#[derive(Deserialize)]
pub(crate) struct RestoreReq {
    file: String,
    confirm: bool,
}

pub(crate) async fn restore_backup(State(st): St, a: Auth, Json(b): Json<RestoreReq>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn get_resources(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.rm.settings();
    Ok(Json(json!({ "profile": s.profile, "overrides": s.overrides, "effective": st.rm.limits() })))
}

pub(crate) async fn put_resources(State(st): St, a: Auth, Json(b): Json<doto_system::StoredResourceSettings>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn get_settings(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

pub(crate) async fn put_settings(State(st): St, a: Auth, Json(b): Json<settings::AppSettings>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn get_onboarding(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let o: settings::OnboardingState = settings::get(&st.db, settings::KEY_ONBOARDING).await?;
    Ok(Json(json!(o)))
}

pub(crate) async fn complete_onboarding(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let now = doto_core::now_rfc3339();
    let o = settings::OnboardingState { completed: true, completed_at: Some(now.clone()), accepted_terms_at: Some(now) };
    settings::set(&st.db, settings::KEY_ONBOARDING, &o).await?;
    audit_ev(&st, &a, "onboarding.complete", None, None).await;
    Ok(Json(json!(o)))
}

pub(crate) async fn events(State(st): St, a: Auth) -> Sse<impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
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

pub(crate) async fn list_audit(State(st): St, a: Auth, Query(q): Query<LimitQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(audit::list(&st.db, q.limit.unwrap_or(200)).await?)))
}

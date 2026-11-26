//! Xác thực & phân quyền: Owner / Viewer, phiên cookie (HttpOnly, SameSite=Strict, Secure khi HTTPS),
//! CSRF token, rate limit, mã ghép nối hết hạn, cờ `hostControl` chỉ có trên listener Control.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{request::Parts, HeaderMap, Method};
use rusqlite::{params, OptionalExtension};

use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenerKind {
    Control,
    Public,
}

impl ListenerKind {
    pub fn cookie_name(&self) -> &'static str {
        match self {
            ListenerKind::Control => "doto_ctl",
            ListenerKind::Public => "doto_session",
        }
    }
}

pub const OWNER_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
pub const VIEWER_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
pub const DESKTOP_TTL: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Clone)]
pub struct Auth {
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub host_control: bool,
    pub session_id: String,
    pub csrf: String,
    pub ip: Option<String>,
}

impl Auth {
    pub fn is_owner(&self) -> bool {
        self.role == "owner"
    }
    pub fn require_owner(&self) -> ApiResult<()> {
        if self.is_owner() {
            Ok(())
        } else {
            Err(ApiError::forbidden("cần quyền Owner"))
        }
    }
    pub fn require_host(&self) -> ApiResult<()> {
        self.require_owner()?;
        if self.host_control {
            Ok(())
        } else {
            Err(ApiError::host_only())
        }
    }
}

// ---------------- Mật khẩu ----------------

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(pw.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("{e}"))?.to_string())
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(h) => Argon2::default().verify_password(pw.as_bytes(), &h).is_ok(),
        Err(_) => false,
    }
}

/// Hash giả để so sánh khi user không tồn tại (cân bằng thời gian, chống dò username).
pub fn dummy_verify(pw: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let h = DUMMY.get_or_init(|| hash_password("dummy-password-123").unwrap_or_default());
    let _ = verify_password(pw, h);
}

pub fn password_policy(pw: &str) -> Result<(), String> {
    let len = pw.chars().count();
    if !(10..=256).contains(&len) {
        return Err("mật khẩu phải dài 10–256 ký tự".into());
    }
    if !pw.chars().any(|c| c.is_alphabetic()) || !pw.chars().any(|c| c.is_ascii_digit()) {
        return Err("mật khẩu phải có cả chữ và số".into());
    }
    Ok(())
}

pub fn username_policy(u: &str) -> Result<(), String> {
    if !(3..=32).contains(&u.len()) || !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err("tên đăng nhập 3–32 ký tự: chữ, số, . _ -".into());
    }
    Ok(())
}

// ---------------- Rate limit ----------------

#[derive(Default)]
pub struct RateLimiter {
    map: Mutex<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    /// true nếu còn được phép (tối đa `max` lần mỗi `window`).
    pub fn check(&self, key: &str, max: u32, window: Duration) -> bool {
        let mut m = self.map.lock().unwrap();
        let now = Instant::now();
        if m.len() > 10_000 {
            m.retain(|_, (_, t)| now.duration_since(*t) < window);
        }
        let e = m.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(e.1) >= window {
            *e = (0, now);
        }
        e.0 += 1;
        e.0 <= max
    }
}

// ---------------- IP / header ----------------

/// IP client: nếu kết nối đến từ loopback (cloudflared cục bộ) thì tin `CF-Connecting-IP` / `X-Forwarded-For`.
pub fn client_ip(peer: Option<SocketAddr>, headers: &HeaderMap) -> Option<IpAddr> {
    let peer_ip = peer.map(|p| p.ip());
    if peer_ip.map(|i| i.is_loopback()).unwrap_or(false) {
        for h in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(v) = headers.get(h).and_then(|v| v.to_str().ok()) {
                if let Some(ip) = v.split(',').next().and_then(|s| s.trim().parse().ok()) {
                    return Some(ip);
                }
            }
        }
    }
    peer_ip
}

/// Request đến qua HTTPS (trực tiếp hoặc qua cloudflared đặt X-Forwarded-Proto).
pub fn is_https(peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    let from_local_proxy = peer.map(|p| p.ip().is_loopback()).unwrap_or(false);
    from_local_proxy && headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) == Some("https")
}

pub fn session_cookie(kind: ListenerKind, token: &str, ttl: Duration, secure: bool) -> String {
    format!("{}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}", kind.cookie_name(), token, ttl.as_secs(), if secure { "; Secure" } else { "" })
}

pub fn clear_cookie(kind: ListenerKind) -> String {
    format!("{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0", kind.cookie_name())
}

fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all("cookie").iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == name && !v.is_empty()).then(|| v.to_string())
    })
}

// ---------------- Phiên ----------------

pub struct NewSession {
    pub token: String,
    pub csrf: String,
    pub id: String,
}

pub async fn create_session(
    st: &AppState,
    user_id: &str,
    device: &str,
    ua: Option<String>,
    ip: Option<String>,
    host_control: bool,
    ttl: Duration,
) -> anyhow::Result<NewSession> {
    let token = doto_core::random_token(32);
    let csrf = doto_core::random_token(24);
    let id = doto_core::new_id();
    let (t, c, i, u) = (doto_core::sha256_hex(&token), csrf.clone(), id.clone(), user_id.to_string());
    let device: String = device.chars().filter(|c| !c.is_control()).take(80).collect();
    let device = if device.trim().is_empty() { "Thiết bị".to_string() } else { device };
    let ua: Option<String> = ua.map(|s| s.chars().take(300).collect());
    let now = chrono::Utc::now();
    let exp = (now + chrono::Duration::from_std(ttl)?).to_rfc3339();
    st.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO sessions(id,token_hash,csrf_token,user_id,device_name,user_agent,ip,host_control,created_at,last_seen_at,expires_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10)",
                params![i, t, c, u, device, ua, ip, host_control as i64, now.to_rfc3339(), exp],
            )?;
            Ok(())
        })
        .await?;
    Ok(NewSession { token, csrf, id })
}

async fn load_session(st: &AppState, token: &str) -> anyhow::Result<Option<Auth>> {
    let th = doto_core::sha256_hex(token);
    st.db
        .call(move |c| {
            let row = c
                .query_row(
                    "SELECT s.id, s.csrf_token, s.host_control, s.expires_at, s.last_seen_at, u.id, u.username, u.role
                     FROM sessions s JOIN users u ON u.id=s.user_id
                     WHERE s.token_hash=?1 AND s.revoked=0 AND u.disabled=0",
                    [th],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, String>(4)?,
                            r.get::<_, String>(5)?,
                            r.get::<_, String>(6)?,
                            r.get::<_, String>(7)?,
                        ))
                    },
                )
                .optional()?;
            let Some((sid, csrf, hc, exp, last, uid, uname, role)) = row else { return Ok(None) };
            let now = chrono::Utc::now();
            let exp = chrono::DateTime::parse_from_rfc3339(&exp).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or(now);
            if exp <= now {
                return Ok(None);
            }
            let last = chrono::DateTime::parse_from_rfc3339(&last).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or(now);
            if (now - last).num_seconds() > 60 {
                c.execute("UPDATE sessions SET last_seen_at=?2 WHERE id=?1", params![sid, now.to_rfc3339()])?;
            }
            Ok(Some(Auth { user_id: uid, username: uname, role, host_control: hc != 0, session_id: sid, csrf, ip: None }))
        })
        .await
}

/// Lấy phiên (nếu có) từ cookie của listener hoặc Bearer token.
pub async fn authenticate(st: &AppState, parts: &Parts) -> ApiResult<Option<Auth>> {
    let kind = parts.extensions.get::<ListenerKind>().copied().unwrap_or(ListenerKind::Public);
    let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let bearer = parts.headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(|s| s.trim().to_string());
    let (token, via_cookie) = match (bearer, read_cookie(&parts.headers, kind.cookie_name())) {
        (Some(b), _) => (b, false),
        (None, Some(c)) => (c, true),
        (None, None) => return Ok(None),
    };
    let Some(mut auth) = load_session(st, &token).await? else { return Ok(None) };
    // hostControl chỉ có hiệu lực trên listener Control (phòng thủ nhiều lớp)
    if kind != ListenerKind::Control {
        auth.host_control = false;
    }
    if via_cookie && !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS) {
        let sent = parts.headers.get("x-csrf-token").and_then(|v| v.to_str().ok()).unwrap_or("");
        if !constant_eq(sent.as_bytes(), auth.csrf.as_bytes()) {
            return Err(ApiError::forbidden("thiếu hoặc sai CSRF token"));
        }
    }
    auth.ip = client_ip(peer, &parts.headers).map(|i| i.to_string());
    Ok(Some(auth))
}

pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl FromRequestParts<Arc<AppState>> for Auth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, st: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        authenticate(st, parts).await?.ok_or_else(ApiError::unauthorized)
    }
}

pub struct MaybeAuth(pub Option<Auth>);

impl FromRequestParts<Arc<AppState>> for MaybeAuth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, st: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuth(authenticate(st, parts).await.unwrap_or(None)))
    }
}

// ---------------- Người dùng ----------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub role: String,
    pub created_at: String,
}

pub async fn find_user(st: &AppState, username: &str) -> anyhow::Result<Option<(UserDto, String, bool)>> {
    let u = username.to_string();
    st.db
        .call(move |c| {
            Ok(c.query_row("SELECT id, username, role, created_at, password_hash, disabled FROM users WHERE username=?1", [u], |r| {
                Ok((UserDto { id: r.get(0)?, username: r.get(1)?, role: r.get(2)?, created_at: r.get(3)? }, r.get::<_, String>(4)?, r.get::<_, i64>(5)? != 0))
            })
            .optional()?)
        })
        .await
}

pub async fn find_user_by_id(st: &AppState, id: &str) -> anyhow::Result<Option<UserDto>> {
    let i = id.to_string();
    st.db
        .call(move |c| {
            Ok(c.query_row("SELECT id, username, role, created_at FROM users WHERE id=?1", [i], |r| {
                Ok(UserDto { id: r.get(0)?, username: r.get(1)?, role: r.get(2)?, created_at: r.get(3)? })
            })
            .optional()?)
        })
        .await
}

/// Owner cục bộ (tạo tự động lần đầu chạy, chưa có mật khẩu → không đăng nhập từ xa được).
pub async fn ensure_local_owner(st: &AppState) -> anyhow::Result<String> {
    st.db
        .call(|c| {
            let existing: Option<String> = c.query_row("SELECT id FROM users WHERE role='owner' ORDER BY created_at LIMIT 1", [], |r| r.get(0)).optional()?;
            if let Some(id) = existing {
                return Ok(id);
            }
            let id = doto_core::new_id();
            c.execute("INSERT INTO users(id,username,password_hash,role,created_at) VALUES(?1,'owner','!','owner',?2)", params![id, doto_core::now_rfc3339()])?;
            Ok(id)
        })
        .await
}

pub async fn owner_password_set(st: &AppState) -> bool {
    st.db
        .call(|c| Ok(c.query_row("SELECT count(*) FROM users WHERE role='owner' AND password_hash<>'!' AND disabled=0", [], |r| r.get::<_, i64>(0))?))
        .await
        .map(|n| n > 0)
        .unwrap_or(false)
}

// ---------------- Mã ghép nối ----------------

const PAIR_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn new_pairing_code() -> String {
    use rand::Rng;
    let mut r = rand::rngs::OsRng;
    (0..8).map(|_| PAIR_ALPHABET[r.gen_range(0..PAIR_ALPHABET.len())] as char).collect()
}

pub fn normalize_code(c: &str) -> String {
    c.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip_and_policy() {
        let h = hash_password("correct horse 123").unwrap();
        assert!(verify_password("correct horse 123", &h));
        assert!(!verify_password("wrong", &h));
        assert!(!verify_password("x", "!"), "hash '!' không bao giờ khớp");
        assert!(password_policy("short1").is_err());
        assert!(password_policy("onlyletterslong").is_err());
        assert!(password_policy("letters and 123").is_ok());
        assert!(username_policy("ab").is_err());
        assert!(username_policy("bad name").is_err());
        assert!(username_policy("viewer_01").is_ok());
    }

    #[test]
    fn rate_limiter_blocks_after_max() {
        let r = RateLimiter::default();
        for _ in 0..3 {
            assert!(r.check("ip", 3, Duration::from_secs(60)));
        }
        assert!(!r.check("ip", 3, Duration::from_secs(60)));
        assert!(r.check("other", 3, Duration::from_secs(60)));
    }

    #[test]
    fn pairing_codes() {
        let c = new_pairing_code();
        assert_eq!(c.len(), 8);
        assert!(!c.contains('0') && !c.contains('O') && !c.contains('1') && !c.contains('I'));
        assert_eq!(normalize_code(" abcd-efgh "), "ABCDEFGH");
    }

    #[test]
    fn client_ip_trusts_forwarded_only_from_loopback() {
        let mut h = HeaderMap::new();
        h.insert("cf-connecting-ip", "203.0.113.9".parse().unwrap());
        let lo: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let lan: SocketAddr = "192.168.1.20:5000".parse().unwrap();
        assert_eq!(client_ip(Some(lo), &h).unwrap().to_string(), "203.0.113.9");
        assert_eq!(client_ip(Some(lan), &h).unwrap().to_string(), "192.168.1.20");
    }

    #[test]
    fn cookie_flags() {
        let c = session_cookie(ListenerKind::Public, "tok", OWNER_TTL, true);
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict") && c.contains("Secure"));
        assert!(!session_cookie(ListenerKind::Control, "t", DESKTOP_TTL, false).contains("Secure"));
    }
}

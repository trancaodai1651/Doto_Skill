//! doto-server: máy chủ web/API cục bộ của Doto Skill (Axum + Tokio).
//!
//! Hai listener dùng chung `AppState`:
//! - **Control** `127.0.0.1:<ngẫu nhiên>`: luôn bật khi app desktop chạy, dùng token một lần cho cửa sổ desktop.
//! - **Public**: Off / Localhost / LAN do người dùng bật/tắt; phục vụ giao diện Web & API với đầy đủ chức năng cho Owner
//!   (chạy trên phần cứng máy chủ qua allowlist an toàn, không thực thi shell/FS tuỳ ý) và giới hạn chặt RBAC cho Viewer.

pub mod auth;
pub mod error;
pub mod public;
pub mod routes;
pub mod stream;

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;

use doto_core::{events::EventBus, paths::AppPaths, secrets::SharedSecrets, settings, Db};
use doto_integrations::{ToolManager, TunnelManager};
use doto_media::MediaService;
use doto_skills::SkillService;
use doto_system::{HardwareInfo, ResourceManager, StoredResourceSettings};

use crate::auth::{ListenerKind, RateLimiter};
use crate::public::{PublicServer, ServerConfig};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AppState {
    pub db: Db,
    pub paths: AppPaths,
    pub events: EventBus,
    pub secrets: SharedSecrets,
    pub rm: ResourceManager,
    pub media: Arc<MediaService>,
    pub skills: Arc<SkillService>,
    pub tools: Arc<ToolManager>,
    pub tunnel: Arc<TunnelManager>,
    pub hardware: RwLock<Option<HardwareInfo>>,
    pub public: PublicServer,
    pub control_addr: RwLock<Option<SocketAddr>>,
    desktop_tokens: Mutex<HashSet<String>>,
    media_tickets: Mutex<HashMap<String, (String, Instant)>>,
    pub rate: RateLimiter,
    pub started: Instant,
    pub web_root: Option<PathBuf>,
    pub github_client_id: Option<String>,
    pub device_flow: Mutex<Option<(String, Instant)>>,
    pub stream_slots: Arc<tokio::sync::Semaphore>,
    /// Số job đang chờ (cập nhật bởi metrics pump) để `metrics()` không phải khoá DB.
    queued_cache: std::sync::atomic::AtomicUsize,
}

pub struct Options {
    pub data_dir: Option<PathBuf>,
    pub web_root: Option<PathBuf>,
    pub secrets: Option<SharedSecrets>,
    pub github_client_id: Option<String>,
    /// Bỏ qua probe phần cứng/CLI lúc khởi động (test nhanh).
    pub skip_background_probes: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            data_dir: None,
            web_root: None,
            secrets: None,
            github_client_id: std::env::var("DOTO_GITHUB_CLIENT_ID").ok().filter(|s| !s.is_empty()),
            skip_background_probes: false,
        }
    }
}

impl AppState {
    pub async fn init(opts: Options) -> anyhow::Result<Arc<Self>> {
        let paths = AppPaths::resolve(opts.data_dir.as_deref())?;
        let db = Db::open(&paths.db_path())?;
        let events = EventBus::default();
        let secrets = opts.secrets.unwrap_or_else(doto_core::secrets::default_store);
        let stored: StoredResourceSettings = settings::get(&db, settings::KEY_RESOURCES).await?;
        let rm = ResourceManager::new(ResourceManager::detect_capacity(), stored);
        let media = MediaService::new(db.clone(), paths.clone(), rm.clone(), events.clone());
        let skills = SkillService::new(db.clone(), paths.clone(), events.clone(), secrets.clone());
        let tools = ToolManager::new(db.clone(), events.clone(), &paths.data_dir);
        let tunnel = TunnelManager::new(secrets.clone(), events.clone());
        let hw: Option<HardwareInfo> = db
            .call(|c| {
                use rusqlite::OptionalExtension;
                let s: Option<String> = c.query_row("SELECT snapshot FROM host_hardware ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).optional()?;
                Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
            })
            .await?;
        let st = Arc::new(Self {
            db,
            paths,
            events,
            secrets,
            rm,
            media,
            skills,
            tools,
            tunnel,
            hardware: RwLock::new(hw),
            public: PublicServer::default(),
            control_addr: RwLock::new(None),
            desktop_tokens: Mutex::new(HashSet::new()),
            media_tickets: Mutex::new(HashMap::new()),
            rate: RateLimiter::default(),
            started: Instant::now(),
            web_root: opts.web_root,
            github_client_id: opts.github_client_id,
            device_flow: Mutex::new(None),
            stream_slots: Arc::new(tokio::sync::Semaphore::new(24)),
            queued_cache: std::sync::atomic::AtomicUsize::new(0),
        });
        auth::ensure_local_owner(&st).await?;
        st.rm.spawn_sampler();
        st.media.refresh_ffmpeg().await;
        st.media.spawn_scheduler();
        st.skills.restore_watchers().await;
        st.clone().spawn_metrics_pump();
        if !opts.skip_background_probes {
            let s2 = st.clone();
            tokio::spawn(async move {
                if s2.hardware.read().unwrap().is_none() {
                    let _ = s2.probe_hardware().await;
                }
                s2.tools.status_all().await;
            });
        }
        Ok(st)
    }

    /// Token một lần cho cửa sổ desktop (đưa vào URL fragment, không qua mạng).
    pub fn issue_desktop_token(&self) -> String {
        let t = doto_core::random_token(32);
        let mut g = self.desktop_tokens.lock().unwrap();
        if g.len() > 16 {
            g.clear();
        }
        g.insert(doto_core::sha256_hex(&t));
        t
    }

    pub fn consume_desktop_token(&self, t: &str) -> bool {
        self.desktop_tokens.lock().unwrap().remove(&doto_core::sha256_hex(t))
    }

    /// Vé ngắn hạn (10 phút) chỉ dùng cho GET media/QR/SSE khi bản Web kết nối chéo miền (cross-origin Bearer)
    /// tới máy chủ của bản App — giúp không bao giờ đưa session token vào URL.
    pub fn issue_media_ticket(&self, session_id: &str) -> String {
        let raw = doto_core::random_token(24);
        let hash = doto_core::sha256_hex(&raw);
        let now = Instant::now();
        let mut g = self.media_tickets.lock().unwrap();
        g.retain(|_, (_, exp)| *exp > now);
        if g.len() > 2048 {
            g.clear();
        }
        g.insert(hash, (session_id.to_string(), now + Duration::from_secs(600)));
        raw
    }

    pub fn verify_media_ticket(&self, raw: &str) -> Option<String> {
        let hash = doto_core::sha256_hex(raw);
        let now = Instant::now();
        let mut g = self.media_tickets.lock().unwrap();
        match g.get(&hash) {
            Some((sid, exp)) if *exp > now => Some(sid.clone()),
            Some(_) => {
                g.remove(&hash);
                None
            }
            None => None,
        }
    }

    pub async fn probe_hardware(&self) -> anyhow::Result<HardwareInfo> {
        let hw = tokio::task::spawn_blocking(doto_system::HardwareProbe::probe).await?;
        let json = serde_json::to_string(&hw)?;
        let ts = hw.probed_at.clone();
        self.db
            .call(move |c| {
                c.execute("INSERT INTO host_hardware(snapshot, probed_at) VALUES(?1, ?2)", rusqlite::params![json, ts])?;
                c.execute("DELETE FROM host_hardware WHERE id NOT IN (SELECT id FROM host_hardware ORDER BY id DESC LIMIT 20)", [])?;
                Ok(())
            })
            .await?;
        *self.hardware.write().unwrap() = Some(hw.clone());
        Ok(hw)
    }

    pub async fn server_config(&self) -> ServerConfig {
        settings::get(&self.db, settings::KEY_SERVER).await.unwrap_or_default()
    }

    /// Số luồng video đồng thời theo profile (giới hạn băng thông/IO cho host).
    pub fn stream_limit(&self) -> usize {
        match self.rm.settings().profile {
            doto_system::Profile::Eco => 3,
            doto_system::Profile::Balanced => 8,
            doto_system::Profile::Max => 24,
        }
    }

    pub fn metrics(&self) -> serde_json::Value {
        let s = self.rm.sample();
        let (running, _) = self.media.running_counts();
        let queued = self.queued_cache.load(Ordering::Relaxed);
        serde_json::json!({
            "ts": doto_core::now_rfc3339(),
            "cpuPercent": s.cpu_percent,
            "memUsedMb": s.mem_used_mb,
            "memTotalMb": s.mem_total_mb,
            "memAvailableMb": s.mem_available_mb,
            "runningJobs": running,
            "queuedJobs": queued,
            "limits": self.rm.limits(),
            "profile": self.rm.settings().profile,
            "throttleReason": self.media.throttle_reason(),
        })
    }

    fn spawn_metrics_pump(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                if let Ok(n) = doto_core::jobs::count_by_status(&self.db, "queued").await {
                    self.queued_cache.store(n as usize, Ordering::Relaxed);
                }
                self.events.emit("metrics", &self.metrics());
                let ms = self.rm.limits().metrics_interval_ms;
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
        });
    }

    /// Bật public server theo cấu hình đã lưu.
    pub async fn start_public(self: &Arc<Self>) -> Result<SocketAddr, String> {
        let cfg = self.server_config().await;
        cfg.validate()?;
        if cfg.mode == "lan" && !auth::owner_password_set(self).await {
            return Err("cần đặt mật khẩu Owner trước khi mở truy cập LAN".into());
        }
        let r = self.public.start(&cfg, build_router(self.clone(), ListenerKind::Public)).await;
        self.events.emit("server", &self.public.status(&cfg));
        r
    }

    pub async fn stop_public(&self) {
        self.public.stop().await;
        let cfg = self.server_config().await;
        self.events.emit("server", &self.public.status(&cfg));
    }

    /// Bật listener Control (desktop). Trả địa chỉ thật.
    pub async fn start_control(self: &Arc<Self>, port: u16) -> anyhow::Result<SocketAddr> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let addr = listener.local_addr()?;
        *self.control_addr.write().unwrap() = Some(addr);
        let app = build_router(self.clone(), ListenerKind::Control).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!(error = %e, "control listener dừng");
            }
        });
        Ok(addr)
    }

    /// Khởi động tự động theo cấu hình (public server + tunnel autoStart).
    pub async fn autostart(self: &Arc<Self>) {
        let cfg = self.server_config().await;
        if cfg.mode != "off" {
            if let Err(e) = self.start_public().await {
                tracing::warn!(error = %e, "không tự bật được web server");
            }
        }
        let tcfg: doto_integrations::TunnelConfig = settings::get(&self.db, settings::KEY_TUNNEL).await.unwrap_or_default();
        if tcfg.auto_start && self.public.is_running() {
            if let (Some(bin), Some(port)) = (self.cloudflared_path(), self.public.local_port()) {
                if let Err(e) = self.tunnel.start(&tcfg, bin, port).await {
                    tracing::warn!(error = %e, "không tự bật được tunnel");
                }
            }
        }
    }

    pub fn cloudflared_path(&self) -> Option<PathBuf> {
        self.tools.locate(&doto_integrations::providers::Cloudflared)
    }

    pub async fn shutdown(&self) {
        self.tunnel.stop().await;
        self.public.stop().await;
    }
}

// ---------------- Middleware ----------------

async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data: blob: http: https:; media-src 'self' blob: http: https:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' http: https:; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    res
}

fn attach_cors(res: &mut Response, origin: Option<&str>) {
    let Some(origin) = origin else { return };
    if let Ok(val) = HeaderValue::from_str(origin) {
        let h = res.headers_mut();
        // Không đặt Access-Control-Allow-Credentials: kết nối cross-origin dùng Authorization: Bearer
        // nên trình duyệt không bao giờ tự đính kèm cookie ngầm (miễn nhiễm CSRF chéo miền).
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
        h.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("Content-Range, Accept-Ranges, Content-Length, Content-Disposition"));
    }
}

/// Gắn loại listener, xử lý CORS cho bản Web kết nối từ xa qua Bearer token, kiểm tra Origin cho request
/// dùng cookie (chống CSRF chéo site), IP allowlist cho listener Public, chỉ nhận loopback cho Control.
async fn guard(State((st, kind)): State<(Arc<AppState>, ListenerKind)>, mut req: Request, next: Next) -> Response {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    match kind {
        ListenerKind::Control => {
            if !peer.map(|p| p.ip().is_loopback()).unwrap_or(false) {
                return StatusCode::FORBIDDEN.into_response();
            }
        }
        ListenerKind::Public => {
            let cfg = st.server_config().await;
            if !public::ip_allowed(&cfg.ip_allowlist, auth::client_ip(peer, req.headers())) {
                return error::ApiError::forbidden("địa chỉ IP không nằm trong danh sách cho phép").into_response();
            }
        }
    }

    let origin = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()).map(String::from);
    let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let cross_origin = origin.as_deref().map(|o| o.trim_start_matches("https://").trim_start_matches("http://") != host).unwrap_or(false);

    if *req.method() == Method::OPTIONS {
        let mut res = StatusCode::NO_CONTENT.into_response();
        attach_cors(&mut res, origin.as_deref());
        let h = res.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"));
        h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("Content-Type, Authorization, X-CSRF-Token, Range"));
        h.insert(header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("600"));
        return res;
    }

    if !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) && cross_origin {
        let path = req.uri().path();
        let has_bearer = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(|v| v.starts_with("Bearer ")).unwrap_or(false);
        let public_bearer_auth_route = matches!(path, "/api/auth/login" | "/api/auth/pair");
        // Chặn tuyệt đối cross-origin nếu không dùng Bearer token (hoặc đang gọi /api/auth/desktop, /api/auth/setup).
        if !has_bearer && !public_bearer_auth_route {
            return error::ApiError::forbidden("Origin không hợp lệ").into_response();
        }
    }

    req.extensions_mut().insert(kind);
    let counter = st.public.connections.clone();
    if kind == ListenerKind::Public {
        counter.fetch_add(1, Ordering::Relaxed);
    }
    let mut res = next.run(req).await;
    if cross_origin {
        attach_cors(&mut res, origin.as_deref());
    }
    if kind != ListenerKind::Public {
        return res;
    }
    // giảm bộ đếm khi body (kể cả stream video/SSE dài) bị drop
    struct Guard(Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::Relaxed);
        }
    }
    let g = Guard(counter);
    let (parts, body) = res.into_parts();
    let stream = futures::StreamExt::map(body.into_data_stream(), move |c| {
        let _ = &g;
        c
    });
    Response::from_parts(parts, axum::body::Body::from_stream(stream))
}

pub fn build_router(st: Arc<AppState>, kind: ListenerKind) -> Router {
    let api = routes::api_router(kind).with_state(st.clone());
    let mut app = Router::new().merge(api);
    if let Some(root) = st.web_root.clone().filter(|r| r.join("index.html").is_file()) {
        let index = root.join("index.html");
        let serve = tower_http::services::ServeDir::new(&root).fallback(tower_http::services::ServeFile::new(index));
        app = app.fallback_service(serve);
    } else {
        app = app.fallback(|| async { (StatusCode::NOT_FOUND, "Doto Skill API đang chạy — chưa có giao diện web (apps/web/dist).").into_response() });
    }
    app.layer(middleware::from_fn_with_state((st, kind), guard))
        .layer(middleware::from_fn(security_headers))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(2 * 1024 * 1024))
}

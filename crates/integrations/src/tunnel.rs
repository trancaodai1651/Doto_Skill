//! TunnelManager: quản lý cả cấu hình đơn mặc định (`/api/tunnel/*`) lẫn trình quản lý CRUD đa tunnel (`/api/tunnels/*`):
//! - `quick`: `cloudflared tunnel --no-autoupdate --url http://127.0.0.1:PORT` → URL tạm `*.trycloudflare.com`.
//! - `token` / `named` (Zero Trust): `cloudflared tunnel --no-autoupdate run` với token truyền qua biến môi trường
//!   `TUNNEL_TOKEN` (không bao giờ đặt trong argv để tránh lộ qua danh sách tiến trình).
//!   Mỗi tunnel lưu token riêng trong OS vault (`cloudflare.tunnel.<id>`); không bao giờ lưu SQLite, log hay trả về API.
//! - Quét và dừng an toàn các tiến trình `cloudflared` bên ngoài trên máy chủ.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use doto_core::{events::EventBus, secrets::SharedSecrets, Db};

pub const TOKEN_KEY: &str = "cloudflare.tunnel.token";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelConfig {
    pub mode: String,
    pub hostname: Option<String>,
    pub auto_start: bool,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self { mode: "quick".into(), hostname: None, auto_start: false }
    }
}

pub fn valid_hostname(h: &str) -> bool {
    h.len() <= 253
        && h.contains('.')
        && h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !l.starts_with('-'))
}

pub fn valid_local_target_url(u: &str) -> bool {
    (u.starts_with("http://") || u.starts_with("https://")) && u.len() <= 200 && !u.chars().any(char::is_whitespace)
}

impl TunnelConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.mode.as_str(), "quick" | "named") {
            return Err("mode phải là quick hoặc named".into());
        }
        if let Some(h) = &self.hostname {
            if !valid_hostname(h) {
                return Err("hostname không hợp lệ (vd: video.example.com)".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub cloudflared_found: bool,
    pub cloudflared_path: Option<String>,
    pub running: bool,
    pub mode: Option<String>,
    pub public_url: Option<String>,
    pub hostname: Option<String>,
    pub has_token: bool,
    pub auto_start: bool,
    pub connected: bool,
    pub started_at: Option<String>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
}

// ---------------- CRUD Đa Tunnel (v0.2) ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedTunnelReq {
    pub name: String,
    /// `quick` | `token` | `named`
    pub kind: String,
    pub target_url: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub cf_tunnel_name: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub notes: Option<String>,
    /// Chỉ gửi khi tạo/cập nhật token; lưu trong OS Vault, không lưu SQLite.
    #[serde(default)]
    pub token: Option<String>,
}

impl ManagedTunnelReq {
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err("tên tunnel phải từ 1 đến 80 ký tự".into());
        }
        if !matches!(self.kind.as_str(), "quick" | "token" | "named") {
            return Err("loại tunnel phải là quick, token hoặc named".into());
        }
        if !valid_local_target_url(self.target_url.trim()) {
            return Err("targetUrl phải bắt đầu bằng http:// hoặc https://".into());
        }
        if let Some(h) = self.hostname.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if !valid_hostname(h) {
                return Err("hostname không hợp lệ (vd: studio.example.com)".into());
            }
        }
        if self.kind == "named" {
            let tn = self.cf_tunnel_name.as_deref().map(str::trim).unwrap_or("");
            if tn.is_empty() || tn.len() > 80 || !tn.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
                return Err("chế độ named yêu cầu tên hoặc UUID tunnel hợp lệ".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedTunnel {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub target_url: String,
    pub hostname: Option<String>,
    pub cf_tunnel_name: Option<String>,
    pub has_token: bool,
    pub auto_start: bool,
    pub notes: String,
    pub running: bool,
    pub connected: bool,
    pub pid: Option<u32>,
    pub public_url: Option<String>,
    pub started_at: Option<String>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCloudflaredProc {
    pub pid: u32,
    pub command_line: String,
    pub target_hint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelsOverview {
    pub cloudflared_found: bool,
    pub cloudflared_path: Option<String>,
    pub default_target_url: String,
    pub managed: Vec<ManagedTunnel>,
    pub external: Vec<ExternalCloudflaredProc>,
}

struct Running {
    child: tokio::process::Child,
    pid: Option<u32>,
    mode: String,
    started_at: String,
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    public_url: Option<String>,
    connected: bool,
    last_error: Option<String>,
    log: VecDeque<String>,
}

pub struct TunnelManager {
    db: Db,
    secrets: SharedSecrets,
    events: EventBus,
    state: Arc<Mutex<State>>,
    managed_states: Mutex<HashMap<String, Arc<Mutex<State>>>>,
}

pub fn extract_quick_url(line: &str) -> Option<String> {
    let re = regex::Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").ok()?;
    re.find(line).map(|m| m.as_str().to_string())
}

fn managed_vault_key(id: &str) -> String {
    format!("cloudflare.tunnel.{id}")
}

fn validate_tunnel_token(raw: &str) -> anyhow::Result<&str> {
    let t = raw.trim();
    anyhow::ensure!(t.len() >= 32 && t.len() <= 4096 && !t.chars().any(char::is_whitespace), "token tunnel không hợp lệ (cần ≥ 32 ký tự, không khoảng trắng)");
    Ok(t)
}

impl TunnelManager {
    pub fn new(secrets: SharedSecrets, events: EventBus) -> Arc<Self> {
        let db = Db::open_in_memory().expect("in-memory sqlite");
        Self::with_db(db, secrets, events)
    }

    pub fn with_db(db: Db, secrets: SharedSecrets, events: EventBus) -> Arc<Self> {
        Arc::new(Self { db, secrets, events, state: Arc::new(Mutex::new(State::default())), managed_states: Mutex::new(HashMap::new()) })
    }

    // ---------- Single tunnel (tương thích v0.1) ----------

    pub fn set_token(&self, token: &str) -> anyhow::Result<()> {
        let t = validate_tunnel_token(token)?;
        self.secrets.set(TOKEN_KEY, t)
    }

    pub fn delete_token(&self) -> anyhow::Result<()> {
        self.secrets.delete(TOKEN_KEY)
    }

    pub fn has_token(&self) -> bool {
        self.secrets.get(TOKEN_KEY).ok().flatten().is_some()
    }

    pub fn status(&self, cfg: &TunnelConfig, cloudflared: Option<PathBuf>) -> TunnelStatus {
        let mut g = self.state.lock().unwrap();
        reap_if_exited(&mut g);
        TunnelStatus {
            cloudflared_found: cloudflared.is_some(),
            cloudflared_path: cloudflared.map(|p| p.to_string_lossy().into_owned()),
            running: g.running.is_some(),
            mode: g.running.as_ref().map(|r| r.mode.clone()),
            public_url: g.public_url.clone(),
            hostname: cfg.hostname.clone(),
            has_token: self.has_token(),
            auto_start: cfg.auto_start,
            connected: g.connected,
            started_at: g.running.as_ref().map(|r| r.started_at.clone()),
            last_error: g.last_error.clone(),
            log_tail: g.log.iter().cloned().collect(),
        }
    }

    pub async fn start(&self, cfg: &TunnelConfig, cloudflared: PathBuf, local_port: u16) -> anyhow::Result<()> {
        cfg.validate().map_err(anyhow::Error::msg)?;
        self.stop().await;
        let mut cmd = crate::manager::command(&cloudflared);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let public_url = match cfg.mode.as_str() {
            "quick" => {
                cmd.args(["tunnel", "--no-autoupdate", "--url", &format!("http://127.0.0.1:{local_port}")]);
                None
            }
            _ => {
                let token =
                    self.secrets.get(TOKEN_KEY)?.ok_or_else(|| anyhow::anyhow!("chưa có token tunnel — tạo tunnel trên Cloudflare Zero Trust và dán token"))?;
                let host = cfg.hostname.clone().ok_or_else(|| anyhow::anyhow!("cần hostname đã gắn với tunnel trên Cloudflare"))?;
                cmd.args(["tunnel", "--no-autoupdate", "run"]).env("TUNNEL_TOKEN", token);
                Some(format!("https://{host}"))
            }
        };
        spawn_into_state(cmd, cfg.mode.clone(), public_url, self.state.clone(), self.events.clone())
    }

    pub async fn stop(&self) {
        stop_state(&self.state).await;
    }

    pub async fn stop_all(&self) {
        self.stop().await;
        let states: Vec<Arc<Mutex<State>>> = self.managed_states.lock().unwrap().values().cloned().collect();
        for st in states {
            stop_state(&st).await;
        }
    }

    // ---------- Multi-tunnel CRUD (v0.2) ----------

    fn state_for(&self, id: &str) -> Arc<Mutex<State>> {
        self.managed_states.lock().unwrap().entry(id.to_string()).or_insert_with(|| Arc::new(Mutex::new(State::default()))).clone()
    }

    pub async fn list_managed(&self) -> anyhow::Result<Vec<ManagedTunnel>> {
        let rows: Vec<DbTunnelRow> = self
            .db
            .call(|c| {
                let mut st = c.prepare(
                    "SELECT id,name,kind,target_url,hostname,cf_tunnel_name,vault_key,auto_start,notes,created_at,updated_at
                     FROM managed_tunnels ORDER BY created_at DESC",
                )?;
                let v = st.query_map([], row_to_tunnel)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            })
            .await?;
        Ok(rows.into_iter().map(|r| self.hydrate_row(r)).collect())
    }

    pub async fn get_managed(&self, id: &str) -> anyhow::Result<Option<ManagedTunnel>> {
        let id2 = id.to_string();
        let row: Option<DbTunnelRow> = self
            .db
            .call(move |c| {
                Ok(c.query_row(
                    "SELECT id,name,kind,target_url,hostname,cf_tunnel_name,vault_key,auto_start,notes,created_at,updated_at
                     FROM managed_tunnels WHERE id=?1",
                    [id2],
                    row_to_tunnel,
                )
                .optional()?)
            })
            .await?;
        Ok(row.map(|r| self.hydrate_row(r)))
    }

    fn hydrate_row(&self, r: DbTunnelRow) -> ManagedTunnel {
        let st = self.state_for(&r.id);
        let mut g = st.lock().unwrap();
        reap_if_exited(&mut g);
        let has_token = r.vault_key.as_deref().and_then(|k| self.secrets.get(k).ok().flatten()).is_some();
        ManagedTunnel {
            id: r.id,
            name: r.name,
            kind: r.kind,
            target_url: r.target_url,
            hostname: r.hostname.clone(),
            cf_tunnel_name: r.cf_tunnel_name,
            has_token,
            auto_start: r.auto_start,
            notes: r.notes,
            running: g.running.is_some(),
            connected: g.connected,
            pid: g.running.as_ref().and_then(|x| x.pid),
            public_url: g.public_url.clone().or_else(|| r.hostname.map(|h| format!("https://{h}"))),
            started_at: g.running.as_ref().map(|x| x.started_at.clone()),
            last_error: g.last_error.clone(),
            log_tail: g.log.iter().cloned().collect(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    pub async fn create_managed(&self, req: ManagedTunnelReq) -> anyhow::Result<ManagedTunnel> {
        req.validate().map_err(anyhow::Error::msg)?;
        let id = doto_core::new_id();
        let vkey = managed_vault_key(&id);
        if let Some(tok) = req.token.as_deref().filter(|t| !t.trim().is_empty()) {
            let clean = validate_tunnel_token(tok)?;
            self.secrets.set(&vkey, clean)?;
        } else if req.kind == "token" {
            anyhow::bail!("chế độ token yêu cầu nhập Cloudflare Tunnel Token");
        }
        let now = doto_core::now_rfc3339();
        let host = req.hostname.map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty());
        let cfname = req.cf_tunnel_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let notes = doto_core::redact_secrets(req.notes.as_deref().unwrap_or("").trim());
        let (id2, name2, kind2, target2, vkey2, now2) =
            (id.clone(), req.name.trim().to_string(), req.kind.clone(), req.target_url.trim().to_string(), vkey, now.clone());
        self.db
            .call(move |c| {
                c.execute(
                    "INSERT INTO managed_tunnels(id,name,kind,target_url,hostname,cf_tunnel_name,vault_key,auto_start,notes,created_at,updated_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
                    params![id2, name2, kind2, target2, host, cfname, vkey2, req.auto_start as i64, notes, now2],
                )?;
                Ok(())
            })
            .await?;
        self.get_managed(&id).await?.ok_or_else(|| anyhow::anyhow!("tunnel vừa tạo không tồn tại"))
    }

    pub async fn update_managed(&self, id: &str, req: ManagedTunnelReq) -> anyhow::Result<ManagedTunnel> {
        req.validate().map_err(anyhow::Error::msg)?;
        let existing = self.get_managed(id).await?.ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
        let vkey = managed_vault_key(id);
        if let Some(tok) = req.token.as_deref().filter(|t| !t.trim().is_empty()) {
            let clean = validate_tunnel_token(tok)?;
            self.secrets.set(&vkey, clean)?;
        } else if req.kind == "token" && !existing.has_token {
            anyhow::bail!("chế độ token yêu cầu nhập Cloudflare Tunnel Token");
        }
        let host = req.hostname.map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty());
        let cfname = req.cf_tunnel_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let notes = doto_core::redact_secrets(req.notes.as_deref().unwrap_or("").trim());
        let (id2, name2, kind2, target2, now) =
            (id.to_string(), req.name.trim().to_string(), req.kind, req.target_url.trim().to_string(), doto_core::now_rfc3339());
        self.db
            .call(move |c| {
                c.execute(
                    "UPDATE managed_tunnels SET name=?2, kind=?3, target_url=?4, hostname=?5, cf_tunnel_name=?6, vault_key=?7, auto_start=?8, notes=?9, updated_at=?10
                     WHERE id=?1",
                    params![id2, name2, kind2, target2, host, cfname, vkey, req.auto_start as i64, notes, now],
                )?;
                Ok(())
            })
            .await?;
        self.get_managed(id).await?.ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))
    }

    pub async fn delete_managed(&self, id: &str) -> anyhow::Result<bool> {
        self.stop_managed(id).await;
        let _ = self.secrets.delete(&managed_vault_key(id));
        self.managed_states.lock().unwrap().remove(id);
        let id2 = id.to_string();
        let n = self.db.call(move |c| Ok(c.execute("DELETE FROM managed_tunnels WHERE id=?1", [id2])?)).await?;
        Ok(n > 0)
    }

    pub async fn start_managed(&self, id: &str, cloudflared: PathBuf) -> anyhow::Result<ManagedTunnel> {
        let t = self.get_managed(id).await?.ok_or_else(|| anyhow::anyhow!("không tìm thấy cấu hình tunnel"))?;
        self.stop_managed(id).await;
        let mut cmd = crate::manager::command(&cloudflared);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let public_url = match t.kind.as_str() {
            "quick" => {
                cmd.args(["tunnel", "--no-autoupdate", "--url", &t.target_url]);
                None
            }
            "token" => {
                let tok = self.secrets.get(&managed_vault_key(id))?.ok_or_else(|| anyhow::anyhow!("chưa có token trong OS vault cho tunnel này"))?;
                cmd.args(["tunnel", "--no-autoupdate", "run"]).env("TUNNEL_TOKEN", tok);
                t.hostname.as_ref().map(|h| format!("https://{h}"))
            }
            _ => {
                let cfname = t.cf_tunnel_name.as_deref().ok_or_else(|| anyhow::anyhow!("thiếu tên tunnel Cloudflare"))?;
                cmd.args(["tunnel", "--no-autoupdate", "--url", &t.target_url, "run", cfname]);
                t.hostname.as_ref().map(|h| format!("https://{h}"))
            }
        };
        let st = self.state_for(id);
        spawn_into_state(cmd, t.kind.clone(), public_url, st, self.events.clone())?;
        self.get_managed(id).await?.ok_or_else(|| anyhow::anyhow!("tunnel biến mất"))
    }

    pub async fn stop_managed(&self, id: &str) {
        let st = self.state_for(id);
        stop_state(&st).await;
    }

    /// Tạo (nếu chưa có) và bật ngay một Quick Tunnel cho cổng Web hiện tại.
    pub async fn start_quick_web(&self, cloudflared: PathBuf, local_port: u16) -> anyhow::Result<ManagedTunnel> {
        let target = format!("http://127.0.0.1:{local_port}");
        let existing = self.list_managed().await?.into_iter().find(|t| t.kind == "quick" && t.target_url == target);
        let id = match existing {
            Some(t) => t.id,
            None => {
                self.create_managed(ManagedTunnelReq {
                    name: format!("Quick Web ({local_port})"),
                    kind: "quick".into(),
                    target_url: target,
                    hostname: None,
                    cf_tunnel_name: None,
                    auto_start: false,
                    notes: Some("Tạo nhanh cho máy chủ Web Doto Skill".into()),
                    token: None,
                })
                .await?
                .id
            }
        };
        self.start_managed(&id, cloudflared).await
    }

    /// Liệt kê các tiến trình `cloudflared` đang chạy trên hệ điều hành mà không do TunnelManager này nắm giữ.
    pub async fn overview(&self, cloudflared: Option<PathBuf>, default_port: u16) -> anyhow::Result<TunnelsOverview> {
        let managed = self.list_managed().await?;
        let mut known_pids: Vec<u32> = managed.iter().filter_map(|m| m.pid).collect();
        if let Some(p) = self.state.lock().unwrap().running.as_ref().and_then(|r| r.pid) {
            known_pids.push(p);
        }
        let external = tokio::task::spawn_blocking(move || scan_external_cloudflared(&known_pids)).await.unwrap_or_default();
        Ok(TunnelsOverview {
            cloudflared_found: cloudflared.is_some(),
            cloudflared_path: cloudflared.map(|p| p.to_string_lossy().into_owned()),
            default_target_url: format!("http://127.0.0.1:{default_port}"),
            managed,
            external,
        })
    }

    /// Dừng một tiến trình `cloudflared` bên ngoài sau khi xác minh đúng tên thực thi `cloudflared` (chống kill PID tuỳ ý).
    pub async fn stop_external(&self, pid: u32) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || {
            let mut sys = sysinfo::System::new();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let spid = sysinfo::Pid::from_u32(pid);
            let proc_info = sys.process(spid).ok_or_else(|| anyhow::anyhow!("không tìm thấy tiến trình PID {pid}"))?;
            let name = proc_info.name().to_string_lossy().to_ascii_lowercase();
            anyhow::ensure!(name == "cloudflared" || name == "cloudflared.exe", "từ chối dừng PID {pid}: không phải tiến trình cloudflared");
            anyhow::ensure!(proc_info.kill(), "không thể dừng tiến trình cloudflared (PID {pid})");
            Ok(())
        })
        .await?
    }
}

struct DbTunnelRow {
    id: String,
    name: String,
    kind: String,
    target_url: String,
    hostname: Option<String>,
    cf_tunnel_name: Option<String>,
    vault_key: Option<String>,
    auto_start: bool,
    notes: String,
    created_at: String,
    updated_at: String,
}

fn row_to_tunnel(r: &rusqlite::Row) -> rusqlite::Result<DbTunnelRow> {
    Ok(DbTunnelRow {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        target_url: r.get(3)?,
        hostname: r.get(4)?,
        cf_tunnel_name: r.get(5)?,
        vault_key: r.get(6)?,
        auto_start: r.get::<_, i64>(7)? != 0,
        notes: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn reap_if_exited(g: &mut State) {
    let exited = g.running.as_mut().and_then(|r| r.child.try_wait().ok().flatten());
    if let Some(code) = exited {
        g.running = None;
        g.connected = false;
        g.public_url = None;
        g.last_error = Some(format!("cloudflared đã thoát (mã {:?})", code.code()));
    }
}

fn spawn_into_state(
    mut cmd: tokio::process::Command,
    mode: String,
    public_url: Option<String>,
    state: Arc<Mutex<State>>,
    events: EventBus,
) -> anyhow::Result<()> {
    let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("không chạy được cloudflared: {e}"))?;
    let pid = child.id();
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    {
        let mut g = state.lock().unwrap();
        g.running = Some(Running { child, pid, mode, started_at: doto_core::now_rfc3339() });
        g.public_url = public_url;
        g.connected = false;
        g.last_error = None;
        g.log.clear();
    }
    for stream in [Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>, Box::new(stderr)] {
        let st = state.clone();
        let ev = events.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let clean = doto_core::redact_secrets(&l);
                let mut g = st.lock().unwrap();
                if let Some(u) = extract_quick_url(&clean) {
                    g.public_url = Some(u);
                }
                if clean.contains("Registered tunnel connection") || clean.contains("Connection registered") {
                    g.connected = true;
                }
                g.log.push_back(clean);
                while g.log.len() > 200 {
                    g.log.pop_front();
                }
                drop(g);
                ev.emit("tunnel", &serde_json::json!({ "changed": true }));
            }
        });
    }
    Ok(())
}

async fn stop_state(state: &Arc<Mutex<State>>) {
    let r = state.lock().unwrap().running.take();
    if let Some(mut r) = r {
        let _ = r.child.kill().await;
    }
    let mut g = state.lock().unwrap();
    g.connected = false;
    g.public_url = None;
}

fn scan_external_cloudflared(managed_pids: &[u32]) -> Vec<ExternalCloudflaredProc> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut out = vec![];
    for (pid, proc_info) in sys.processes() {
        let p = pid.as_u32();
        if managed_pids.contains(&p) {
            continue;
        }
        let name = proc_info.name().to_string_lossy().to_ascii_lowercase();
        if name != "cloudflared" && name != "cloudflared.exe" {
            continue;
        }
        let raw_cmd = proc_info.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ");
        let clean = doto_core::redact_secrets(if raw_cmd.is_empty() { "cloudflared" } else { &raw_cmd });
        let hint = parse_target_hint(&clean);
        out.push(ExternalCloudflaredProc { pid: p, command_line: clean, target_hint: hint });
    }
    out.sort_by_key(|x| x.pid);
    out
}

fn parse_target_hint(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    for w in parts.windows(2) {
        if w[0] == "--url" {
            return w[1].trim_matches(['"', '\'']).to_string();
        }
    }
    if cmd.contains("--token") {
        return "Zero Trust Token Tunnel".into();
    }
    "Cloudflare Tunnel".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quick_url_from_log() {
        let l = "2026-11-26T10:00:00Z INF |  https://shy-river-1234.trycloudflare.com                                   |";
        assert_eq!(extract_quick_url(l).as_deref(), Some("https://shy-river-1234.trycloudflare.com"));
        assert_eq!(extract_quick_url("https://evil.com/trycloudflare.com"), None);
    }

    #[test]
    fn config_validation() {
        assert!(TunnelConfig::default().validate().is_ok());
        let c = TunnelConfig { mode: "named".into(), hostname: Some("video.example.com".into()), auto_start: true };
        assert!(c.validate().is_ok());
        let c = TunnelConfig { mode: "named".into(), hostname: Some("bad host;rm".into()), auto_start: false };
        assert!(c.validate().is_err());
        let c = TunnelConfig { mode: "ssh".into(), hostname: None, auto_start: false };
        assert!(c.validate().is_err());
    }

    #[tokio::test]
    async fn token_is_stored_in_vault_and_validated() {
        let m = TunnelManager::new(Arc::new(doto_core::secrets::MemoryStore::default()), EventBus::default());
        assert!(!m.has_token());
        assert!(m.set_token("short").is_err());
        m.set_token(&"e".repeat(150)).unwrap();
        assert!(m.has_token());
        let st = m.status(&TunnelConfig::default(), None);
        assert!(st.has_token && !st.running);
        assert!(!serde_json::to_string(&st).unwrap().contains("eeee"), "token không bao giờ xuất hiện trong status");
        m.delete_token().unwrap();
        assert!(!m.has_token());
    }

    #[tokio::test]
    async fn named_mode_without_token_fails_cleanly() {
        let m = TunnelManager::new(Arc::new(doto_core::secrets::MemoryStore::default()), EventBus::default());
        let c = TunnelConfig { mode: "named".into(), hostname: Some("a.example.com".into()), auto_start: false };
        let e = m.start(&c, PathBuf::from("cloudflared"), 8080).await.unwrap_err();
        assert!(e.to_string().contains("token"));
    }

    #[tokio::test]
    async fn managed_tunnels_crud_and_vault_isolation() {
        let secrets = Arc::new(doto_core::secrets::MemoryStore::default());
        let db = Db::open_in_memory().unwrap();
        let m = TunnelManager::with_db(db.clone(), secrets, EventBus::default());

        let tok = "t".repeat(120);
        let t1 = m
            .create_managed(ManagedTunnelReq {
                name: "Prod Studio".into(),
                kind: "token".into(),
                target_url: "http://127.0.0.1:4380".into(),
                hostname: Some("studio.example.com".into()),
                cf_tunnel_name: None,
                auto_start: true,
                notes: Some("Main".into()),
                token: Some(tok.clone()),
            })
            .await
            .unwrap();
        assert!(t1.has_token);
        let serialized = serde_json::to_string(&t1).unwrap();
        assert!(!serialized.contains(&tok), "JSON trả về không được chứa token");

        let dump: String = db
            .call(|c| Ok(c.query_row("SELECT name || '|' || coalesce(vault_key,'') || '|' || notes FROM managed_tunnels LIMIT 1", [], |r| r.get(0))?))
            .await
            .unwrap();
        assert!(!dump.contains(&tok));

        let t2 = m
            .update_managed(
                &t1.id,
                ManagedTunnelReq {
                    name: "Prod Studio Updated".into(),
                    kind: "token".into(),
                    target_url: "http://127.0.0.1:4380".into(),
                    hostname: Some("studio.example.com".into()),
                    cf_tunnel_name: None,
                    auto_start: false,
                    notes: Some("Updated".into()),
                    token: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(t2.name, "Prod Studio Updated");
        assert!(t2.has_token);

        assert!(m.delete_managed(&t1.id).await.unwrap());
        assert!(m.list_managed().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn stop_external_refuses_non_cloudflared_pid() {
        let m = TunnelManager::new(Arc::new(doto_core::secrets::MemoryStore::default()), EventBus::default());
        let my_pid = std::process::id();
        let err = m.stop_external(my_pid).await.unwrap_err();
        assert!(err.to_string().contains("cloudflared"), "{err}");
    }
}

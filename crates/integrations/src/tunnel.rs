//! TunnelManager: chạy cloudflared outbound (không mở cổng inbound trên router).
//! - quick: `cloudflared tunnel --url http://127.0.0.1:PORT` → URL tạm *.trycloudflare.com (chỉ để thử;
//!   không hỗ trợ SSE, giới hạn 200 request đồng thời, không đảm bảo uptime).
//! - named (production): tunnel quản lý từ xa trên Cloudflare dashboard, chạy `cloudflared tunnel run`
//!   với token truyền qua biến môi trường `TUNNEL_TOKEN` (không đặt trong argv để tránh lộ qua danh sách tiến trình).
//!   Token lưu trong OS vault; không bao giờ ghi log hay trả về API.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use doto_core::{events::EventBus, secrets::SharedSecrets};

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

impl TunnelConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.mode.as_str(), "quick" | "named") {
            return Err("mode phải là quick hoặc named".into());
        }
        if let Some(h) = &self.hostname {
            let ok = h.len() <= 253
                && h.contains('.')
                && h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !l.starts_with('-'));
            if !ok {
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

struct Running {
    child: tokio::process::Child,
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
    secrets: SharedSecrets,
    events: EventBus,
    state: Arc<Mutex<State>>,
}

pub fn extract_quick_url(line: &str) -> Option<String> {
    let re = regex::Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").ok()?;
    re.find(line).map(|m| m.as_str().to_string())
}

impl TunnelManager {
    pub fn new(secrets: SharedSecrets, events: EventBus) -> Arc<Self> {
        Arc::new(Self { secrets, events, state: Arc::new(Mutex::new(State::default())) })
    }

    pub fn set_token(&self, token: &str) -> anyhow::Result<()> {
        let t = token.trim();
        anyhow::ensure!(t.len() >= 32 && t.len() <= 4096 && !t.chars().any(char::is_whitespace), "token tunnel không hợp lệ");
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
        // phát hiện tiến trình đã thoát
        let exited = g.running.as_mut().and_then(|r| r.child.try_wait().ok().flatten());
        if let Some(code) = exited {
            g.running = None;
            g.connected = false;
            g.public_url = None;
            g.last_error = Some(format!("cloudflared đã thoát (mã {:?})", code.code()));
        }
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
                // Public hostname → service http://127.0.0.1:PORT được cấu hình trên dashboard Cloudflare.
                cmd.args(["tunnel", "--no-autoupdate", "run"]).env("TUNNEL_TOKEN", token);
                Some(format!("https://{host}"))
            }
        };
        let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("không chạy được cloudflared: {e}"))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        {
            let mut g = self.state.lock().unwrap();
            g.running = Some(Running { child, mode: cfg.mode.clone(), started_at: doto_core::now_rfc3339() });
            g.public_url = public_url;
            g.connected = false;
            g.last_error = None;
            g.log.clear();
        }
        for stream in [Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>, Box::new(stderr)] {
            let st = self.state.clone();
            let events = self.events.clone();
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
                    events.emit("tunnel", &serde_json::json!({ "changed": true }));
                }
            });
        }
        Ok(())
    }

    pub async fn stop(&self) {
        let r = self.state.lock().unwrap().running.take();
        if let Some(mut r) = r {
            let _ = r.child.kill().await;
        }
        let mut g = self.state.lock().unwrap();
        g.connected = false;
        g.public_url = None;
    }
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
}

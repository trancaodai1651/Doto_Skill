//! Listener Public: Off / Localhost / LAN, start/stop/restart có graceful shutdown, đếm kết nối,
//! IP allowlist. Mặc định Off; không UPnP, không tự mở firewall.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    pub mode: String,
    pub port: u16,
    pub lan_interface_ip: Option<String>,
    pub ip_allowlist: Vec<String>,
    pub public_base_url: Option<String>,
    /// Giới hạn băng thông mỗi luồng video (kbps); 0 = theo profile.
    pub max_stream_kbps: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { mode: "off".into(), port: 4380, lan_interface_ip: None, ip_allowlist: vec![], public_base_url: None, max_stream_kbps: 0 }
    }
}

impl ServerConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.mode.as_str(), "off" | "localhost" | "lan") {
            return Err("chế độ phải là off, localhost hoặc lan".into());
        }
        if self.port < 1024 {
            return Err("cổng phải ≥ 1024 (không cần quyền quản trị)".into());
        }
        if let Some(ip) = &self.lan_interface_ip {
            ip.parse::<IpAddr>().map_err(|_| "IP interface không hợp lệ".to_string())?;
        }
        for c in &self.ip_allowlist {
            parse_cidr(c).ok_or_else(|| format!("CIDR không hợp lệ: {c}"))?;
        }
        if let Some(u) = &self.public_base_url {
            if !(u.starts_with("https://") || u.starts_with("http://")) || u.len() > 200 {
                return Err("publicBaseUrl phải bắt đầu bằng https:// hoặc http://".into());
            }
        }
        Ok(())
    }

    pub fn bind_addr(&self) -> Option<SocketAddr> {
        match self.mode.as_str() {
            "localhost" => Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port)),
            "lan" => {
                let ip = self.lan_interface_ip.as_deref().and_then(|s| s.parse().ok()).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                Some(SocketAddr::new(ip, self.port))
            }
            _ => None,
        }
    }
}

pub fn parse_cidr(s: &str) -> Option<ipnet::IpNet> {
    s.parse::<ipnet::IpNet>().ok().or_else(|| s.parse::<IpAddr>().ok().map(ipnet::IpNet::from))
}

pub fn ip_allowed(list: &[String], ip: Option<IpAddr>) -> bool {
    if list.is_empty() {
        return true;
    }
    let Some(ip) = ip else { return false };
    if ip.is_loopback() {
        return true;
    }
    list.iter().filter_map(|c| parse_cidr(c)).any(|n| n.contains(&ip))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanAddress {
    pub interface: String,
    pub ip: String,
    /// private (RFC1918) | link_local | cgnat (vd Tailscale 100.64/10) | public
    pub scope: &'static str,
}

pub fn ipv4_scope(v4: std::net::Ipv4Addr) -> &'static str {
    let o = v4.octets();
    if v4.is_private() {
        "private"
    } else if v4.is_link_local() {
        "link_local"
    } else if o[0] == 100 && (64..128).contains(&o[1]) {
        "cgnat"
    } else {
        "public"
    }
}

/// Mọi IPv4 không phải loopback, kèm phạm vi — UI cảnh báo khi bind 0.0.0.0 trên máy có IP public.
pub fn lan_addresses() -> Vec<LanAddress> {
    let mut v: Vec<LanAddress> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) => Some(LanAddress { interface: i.name.clone(), ip: v4.to_string(), scope: ipv4_scope(v4) }),
            _ => None,
        })
        .collect();
    v.sort_by_key(|a| match a.scope {
        "private" => 0,
        "cgnat" => 1,
        "link_local" => 2,
        _ => 3,
    });
    v
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub mode: String,
    pub bind_address: Option<String>,
    pub port: u16,
    pub urls: Vec<String>,
    pub lan_addresses: Vec<LanAddress>,
    pub active_connections: usize,
    pub started_at: Option<String>,
    pub pid: u32,
    pub last_error: Option<String>,
}

struct Running {
    addr: SocketAddr,
    mode: String,
    started_at: String,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub struct PublicServer {
    running: Mutex<Option<Running>>,
    last_error: Mutex<Option<String>>,
    pub connections: Arc<AtomicUsize>,
}

impl PublicServer {
    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().as_ref().map(|r| !r.task.is_finished()).unwrap_or(false)
    }

    pub fn local_port(&self) -> Option<u16> {
        self.running.lock().unwrap().as_ref().map(|r| r.addr.port())
    }

    pub fn status(&self, cfg: &ServerConfig) -> ServerStatus {
        let g = self.running.lock().unwrap();
        let lan = lan_addresses();
        let (running, bind, started, mode) = match g.as_ref().filter(|r| !r.task.is_finished()) {
            Some(r) => (true, Some(r.addr), Some(r.started_at.clone()), r.mode.clone()),
            None => (false, None, None, cfg.mode.clone()),
        };
        let port = bind.map(|b| b.port()).unwrap_or(cfg.port);
        let mut urls = vec![];
        if let Some(b) = bind {
            if b.ip().is_unspecified() {
                urls.push(format!("http://127.0.0.1:{port}"));
                urls.extend(lan.iter().map(|l| format!("http://{}:{port}", l.ip)));
            } else {
                urls.push(format!("http://{}:{port}", b.ip()));
            }
            if let Some(p) = &cfg.public_base_url {
                urls.push(p.trim_end_matches('/').to_string());
            }
        }
        ServerStatus {
            running,
            mode,
            bind_address: bind.map(|b| b.to_string()),
            port,
            urls,
            lan_addresses: lan,
            active_connections: self.connections.load(Ordering::Relaxed),
            started_at: started,
            pid: std::process::id(),
            last_error: self.last_error.lock().unwrap().clone(),
        }
    }

    /// Bind cổng rồi phục vụ router. Báo lỗi rõ ràng khi trùng cổng.
    pub async fn start(&self, cfg: &ServerConfig, router: axum::Router) -> Result<SocketAddr, String> {
        self.stop().await;
        let addr = cfg.bind_addr().ok_or_else(|| "chế độ Off — chọn Localhost hoặc LAN".to_string())?;
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::AddrInUse {
                    format!("cổng {} đang được ứng dụng khác sử dụng — chọn cổng khác", addr.port())
                } else {
                    format!("không mở được {addr}: {e}")
                };
                *self.last_error.lock().unwrap() = Some(msg.clone());
                return Err(msg);
            }
        };
        let bound = listener.local_addr().map_err(|e| e.to_string())?;
        let (tx, rx) = oneshot::channel::<()>();
        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        let task = tokio::spawn(async move {
            let res = axum::serve(listener, svc)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
            if let Err(e) = res {
                tracing::error!(error = %e, "public server dừng do lỗi");
            }
        });
        *self.last_error.lock().unwrap() = None;
        *self.running.lock().unwrap() = Some(Running { addr: bound, mode: cfg.mode.clone(), started_at: doto_core::now_rfc3339(), shutdown: tx, task });
        tracing::info!(%bound, mode = %cfg.mode, "public web server đã bật");
        Ok(bound)
    }

    pub async fn stop(&self) {
        let r = self.running.lock().unwrap().take();
        if let Some(r) = r {
            let _ = r.shutdown.send(());
            let mut task = r.task;
            if tokio::time::timeout(std::time::Duration::from_secs(5), &mut task).await.is_err() {
                task.abort();
            }
            tracing::info!("public web server đã tắt");
        }
    }
}

/// Hướng dẫn firewall đúng phạm vi (chỉ mạng Private, chỉ cổng đã chọn). Không tự chạy.
pub fn firewall_guide(port: u16) -> serde_json::Value {
    if cfg!(windows) {
        serde_json::json!({
            "os": "windows",
            "steps": [
                "Lần đầu bật chế độ LAN, Windows Defender Firewall có thể hỏi quyền cho Doto Skill — chỉ tích \"Private networks\", KHÔNG chọn Public.",
                "Đảm bảo mạng Wi-Fi/Ethernet hiện tại được đặt là Private (Settings › Network & internet › Properties).",
                "Nếu thiết bị khác vẫn không truy cập được, tạo rule thủ công bằng PowerShell chạy với quyền Administrator (lệnh bên dưới).",
                "Không mở cổng trên router (port forwarding) — dùng Cloudflare Tunnel nếu cần truy cập qua Internet."
            ],
            "commands": [format!("New-NetFirewallRule -DisplayName \"Doto Skill LAN {port}\" -Direction Inbound -Protocol TCP -LocalPort {port} -Action Allow -Profile Private")]
        })
    } else {
        serde_json::json!({
            "os": "macos",
            "steps": [
                "Nếu Firewall của macOS đang bật, lần đầu chế độ LAN sẽ hỏi \"Allow incoming connections\" cho Doto Skill — chọn Allow.",
                "Kiểm tra tại System Settings › Network › Firewall › Options.",
                "Không mở cổng trên router — dùng Cloudflare Tunnel nếu cần truy cập qua Internet."
            ],
            "commands": ["/usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate"]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_validation_and_bind() {
        let mut c = ServerConfig::default();
        assert!(c.validate().is_ok());
        assert!(c.bind_addr().is_none(), "mặc định Off");
        c.mode = "localhost".into();
        assert_eq!(c.bind_addr().unwrap().to_string(), "127.0.0.1:4380");
        assert_eq!(ipv4_scope("100.66.7.40".parse().unwrap()), "cgnat");
        assert_eq!(ipv4_scope("172.121.50.45".parse().unwrap()), "public");
        assert_eq!(ipv4_scope("192.168.1.2".parse().unwrap()), "private");
        c.mode = "lan".into();
        assert_eq!(c.bind_addr().unwrap().ip().to_string(), "0.0.0.0");
        c.port = 80;
        assert!(c.validate().is_err());
        c.port = 5000;
        c.ip_allowlist = vec!["10.0.0.0/8".into(), "nonsense".into()];
        assert!(c.validate().is_err());
    }

    #[test]
    fn allowlist_matching() {
        let l = vec!["192.168.1.0/24".to_string(), "10.0.0.5".to_string()];
        assert!(ip_allowed(&l, "192.168.1.77".parse().ok()));
        assert!(ip_allowed(&l, "10.0.0.5".parse().ok()));
        assert!(!ip_allowed(&l, "10.0.0.6".parse().ok()));
        assert!(ip_allowed(&l, "127.0.0.1".parse().ok()));
        assert!(ip_allowed(&[], "8.8.8.8".parse().ok()));
    }

    #[tokio::test]
    async fn start_stop_and_port_conflict() {
        let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = blocker.local_addr().unwrap().port();
        let ps = PublicServer::default();
        let cfg = ServerConfig { mode: "localhost".into(), port: busy, ..Default::default() };
        let err = ps.start(&cfg, axum::Router::new()).await.unwrap_err();
        assert!(err.contains("đang được"), "{err}");
        assert!(!ps.is_running());
        let cfg = ServerConfig { mode: "localhost".into(), port: 0, ..Default::default() };
        let mut cfg2 = cfg.clone();
        cfg2.port = 0;
        let addr = ps.start(&cfg2, axum::Router::new()).await.unwrap();
        assert!(ps.is_running());
        assert!(addr.port() > 0);
        ps.stop().await;
        assert!(!ps.is_running());
    }
}

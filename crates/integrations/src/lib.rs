//! doto-integrations: CliProvider + adapter Antigravity / Codex / Claude Code / cloudflared,
//! trình cài có xác nhận và TunnelManager.
pub mod cloudflared_install;
pub mod manager;
pub mod providers;
pub mod tunnel;

pub use manager::{ToolManager, ToolStatus};
pub use tunnel::{TunnelConfig, TunnelManager, TunnelStatus};

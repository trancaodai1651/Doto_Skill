//! doto-integrations: CliProvider + adapter Antigravity / Codex / Claude Code / cloudflared,
//! trình cài có xác nhận, AI Studio Runner và TunnelManager.
pub mod cloudflared_install;
pub mod manager;
pub mod providers;
pub mod studio;
pub mod tunnel;

pub use manager::{ToolManager, ToolStatus};
pub use studio::{StudioModelInfo, StudioProviderCatalog, StudioRunDetail, StudioRunRequest, StudioRunner, StudioSession, StudioSessionSpec};
pub use tunnel::{ExternalCloudflaredProc, ManagedTunnel, ManagedTunnelReq, TunnelConfig, TunnelManager, TunnelStatus, TunnelsOverview};

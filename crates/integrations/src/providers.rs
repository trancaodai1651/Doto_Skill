//! `CliProvider`: interface chung + adapter riêng cho Antigravity, Codex, Claude Code, cloudflared.
//! Mọi lệnh cài lấy từ tài liệu chính thức (kiểm tra 2026-11-26, xem docs/INTEGRATIONS.md).
//! Không lệnh nào tự chạy khi chưa có xác nhận của người dùng.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Other,
}

pub fn current_os() -> Os {
    if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Other
    }
}

/// Cách cài: lệnh shell chính thức, hoặc tải binary có xác minh checksum (cloudflared Windows).
#[derive(Debug, Clone, PartialEq)]
pub enum InstallKind {
    Shell { command: String, requires: Option<&'static str> },
    VerifiedDownload,
}

#[derive(Debug, Clone)]
pub struct InstallSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: InstallKind,
    pub source_url: &'static str,
    pub requires_elevation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    LoggedIn,
    LoggedOut,
    Unknown,
    NotApplicable,
}

pub trait CliProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    fn docs_url(&self) -> &'static str;
    fn common_locations(&self, home: &std::path::Path) -> Vec<PathBuf>;
    fn install_methods(&self, os: Os) -> Vec<InstallSpec>;
    /// Lệnh kiểm tra đăng nhập chính thức (nếu có) — đối số cố định.
    fn auth_args(&self) -> Option<&'static [&'static str]> {
        None
    }
    /// Diễn giải kết quả kiểm tra đăng nhập.
    fn interpret_auth(&self, _exit_ok: bool, _stdout: &str) -> (AuthState, Option<String>) {
        (AuthState::Unknown, Some("công cụ không có lệnh kiểm tra đăng nhập chính thức".into()))
    }
    /// Thư mục skills cấp người dùng mà công cụ đọc.
    fn skills_dir(&self, _home: &std::path::Path) -> Option<PathBuf> {
        None
    }
    /// Allowlist đối số cho chẩn đoán — browser/UI không truyền được lệnh tuỳ ý.
    fn diagnostic_commands(&self) -> Vec<&'static [&'static str]> {
        let mut v: Vec<&'static [&'static str]> = vec![&["--version"]];
        if let Some(a) = self.auth_args() {
            v.push(a);
        }
        v
    }
    fn platform_supported(&self, os: Os) -> bool {
        matches!(os, Os::Windows | Os::MacOs)
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn npm_global_bins(home: &std::path::Path, name: &str) -> Vec<PathBuf> {
    let mut v = vec![];
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            v.push(PathBuf::from(appdata).join("npm").join(format!("{name}.cmd")));
        }
    } else {
        v.push(home.join(".npm-global").join("bin").join(name));
    }
    v
}

fn brew_bins(name: &str) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![PathBuf::from("/opt/homebrew/bin").join(name), PathBuf::from("/usr/local/bin").join(name)]
    } else {
        vec![]
    }
}

// ---------------- Claude Code ----------------
pub struct ClaudeCode;
impl CliProvider for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn name(&self) -> &'static str {
        "Anthropic Claude Code"
    }
    fn binary(&self) -> &'static str {
        "claude"
    }
    fn docs_url(&self) -> &'static str {
        "https://code.claude.com/docs/en/setup"
    }
    fn common_locations(&self, home: &std::path::Path) -> Vec<PathBuf> {
        let mut v = vec![home.join(".local").join("bin").join(exe("claude"))];
        v.extend(npm_global_bins(home, "claude"));
        v.extend(brew_bins("claude"));
        v
    }
    fn install_methods(&self, os: Os) -> Vec<InstallSpec> {
        let src = "https://code.claude.com/docs/en/setup";
        match os {
            Os::Windows => vec![
                InstallSpec {
                    id: "native-ps",
                    label: "Trình cài chính thức (PowerShell)",
                    kind: InstallKind::Shell { command: "irm https://claude.ai/install.ps1 | iex".into(), requires: None },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "winget",
                    label: "WinGet (Anthropic.ClaudeCode)",
                    kind: InstallKind::Shell {
                        command: "winget install --id Anthropic.ClaudeCode -e --accept-source-agreements --accept-package-agreements".into(),
                        requires: Some("winget"),
                    },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "npm",
                    label: "npm (@anthropic-ai/claude-code, cần Node 22+)",
                    kind: InstallKind::Shell { command: "npm install -g @anthropic-ai/claude-code@latest".into(), requires: Some("npm") },
                    source_url: src,
                    requires_elevation: false,
                },
            ],
            Os::MacOs => vec![
                InstallSpec {
                    id: "native-sh",
                    label: "Trình cài chính thức (shell)",
                    kind: InstallKind::Shell { command: "curl -fsSL https://claude.ai/install.sh | bash".into(), requires: Some("curl") },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "brew",
                    label: "Homebrew cask (claude-code)",
                    kind: InstallKind::Shell { command: "brew install --cask claude-code".into(), requires: Some("brew") },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "npm",
                    label: "npm (@anthropic-ai/claude-code)",
                    kind: InstallKind::Shell { command: "npm install -g @anthropic-ai/claude-code@latest".into(), requires: Some("npm") },
                    source_url: src,
                    requires_elevation: false,
                },
            ],
            Os::Other => vec![],
        }
    }
    fn auth_args(&self) -> Option<&'static [&'static str]> {
        Some(&["auth", "status"])
    }
    fn interpret_auth(&self, exit_ok: bool, stdout: &str) -> (AuthState, Option<String>) {
        // `claude auth status`: JSON, exit 0 = đã đăng nhập, 1 = chưa (tài liệu cli-reference)
        let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_default();
        let method = v["authMethod"].as_str().map(|m| format!("phương thức: {m}"));
        match (exit_ok, v["loggedIn"].as_bool()) {
            (_, Some(true)) | (true, None) => (AuthState::LoggedIn, method),
            (_, Some(false)) | (false, None) => (AuthState::LoggedOut, Some("chạy `claude` trong terminal rồi làm theo hướng dẫn đăng nhập".into())),
        }
    }
    fn skills_dir(&self, home: &std::path::Path) -> Option<PathBuf> {
        Some(home.join(".claude").join("skills"))
    }
}

// ---------------- OpenAI Codex ----------------
pub struct Codex;
impl CliProvider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn name(&self) -> &'static str {
        "OpenAI Codex CLI"
    }
    fn binary(&self) -> &'static str {
        "codex"
    }
    fn docs_url(&self) -> &'static str {
        "https://github.com/openai/codex"
    }
    fn common_locations(&self, home: &std::path::Path) -> Vec<PathBuf> {
        let mut v = npm_global_bins(home, "codex");
        v.push(home.join(".local").join("bin").join(exe("codex")));
        v.extend(brew_bins("codex"));
        v
    }
    fn install_methods(&self, os: Os) -> Vec<InstallSpec> {
        let src = "https://github.com/openai/codex";
        match os {
            Os::Windows => vec![
                InstallSpec {
                    id: "native-ps",
                    label: "Trình cài chính thức (PowerShell)",
                    kind: InstallKind::Shell { command: "irm https://chatgpt.com/codex/install.ps1 | iex".into(), requires: None },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "npm",
                    label: "npm (@openai/codex)",
                    kind: InstallKind::Shell { command: "npm install -g @openai/codex".into(), requires: Some("npm") },
                    source_url: src,
                    requires_elevation: false,
                },
            ],
            Os::MacOs => vec![
                InstallSpec {
                    id: "native-sh",
                    label: "Trình cài chính thức (shell)",
                    kind: InstallKind::Shell { command: "curl -fsSL https://chatgpt.com/codex/install.sh | sh".into(), requires: Some("curl") },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "brew",
                    label: "Homebrew cask (codex)",
                    kind: InstallKind::Shell { command: "brew install --cask codex".into(), requires: Some("brew") },
                    source_url: src,
                    requires_elevation: false,
                },
                InstallSpec {
                    id: "npm",
                    label: "npm (@openai/codex)",
                    kind: InstallKind::Shell { command: "npm install -g @openai/codex".into(), requires: Some("npm") },
                    source_url: src,
                    requires_elevation: false,
                },
            ],
            Os::Other => vec![],
        }
    }
    fn auth_args(&self) -> Option<&'static [&'static str]> {
        Some(&["login", "status"])
    }
    fn interpret_auth(&self, exit_ok: bool, stdout: &str) -> (AuthState, Option<String>) {
        // Định dạng output không được tài liệu hoá → chỉ khẳng định khi thông điệp rõ ràng.
        let s = stdout.trim();
        let lower = s.to_ascii_lowercase();
        if lower.contains("not logged in") {
            (AuthState::LoggedOut, Some("chạy `codex login` trong terminal".into()))
        } else if exit_ok && lower.contains("logged in") {
            (AuthState::LoggedIn, Some(s.lines().next().unwrap_or("").chars().take(120).collect()))
        } else {
            (AuthState::Unknown, Some("không diễn giải được `codex login status`".into()))
        }
    }
    fn skills_dir(&self, home: &std::path::Path) -> Option<PathBuf> {
        Some(home.join(".agents").join("skills"))
    }
}

// ---------------- Google Antigravity CLI ----------------
pub struct Antigravity;
impl CliProvider for Antigravity {
    fn id(&self) -> &'static str {
        "antigravity"
    }
    fn name(&self) -> &'static str {
        "Google Antigravity CLI"
    }
    fn binary(&self) -> &'static str {
        "agy"
    }
    fn docs_url(&self) -> &'static str {
        "https://antigravity.google/docs/install?tab=cli"
    }
    fn common_locations(&self, home: &std::path::Path) -> Vec<PathBuf> {
        let mut v = vec![];
        if cfg!(windows) {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                v.push(PathBuf::from(local).join("agy").join("bin").join("agy.exe"));
            }
        }
        v.push(home.join(".local").join("bin").join(exe("agy")));
        v
    }
    fn install_methods(&self, os: Os) -> Vec<InstallSpec> {
        let src = "https://antigravity.google/docs/install?tab=cli";
        match os {
            Os::Windows => vec![InstallSpec {
                id: "native-ps",
                label: "Trình cài chính thức (PowerShell)",
                kind: InstallKind::Shell { command: "irm https://antigravity.google/cli/install.ps1 | iex".into(), requires: None },
                source_url: src,
                requires_elevation: false,
            }],
            Os::MacOs => vec![InstallSpec {
                id: "native-sh",
                label: "Trình cài chính thức (shell)",
                kind: InstallKind::Shell { command: "curl -fsSL https://antigravity.google/cli/install.sh | bash".into(), requires: Some("curl") },
                source_url: src,
                requires_elevation: false,
            }],
            Os::Other => vec![],
        }
    }
    fn skills_dir(&self, home: &std::path::Path) -> Option<PathBuf> {
        Some(home.join(".gemini").join("config").join("skills"))
    }
}

// ---------------- Cloudflare cloudflared ----------------
pub struct Cloudflared;
impl CliProvider for Cloudflared {
    fn id(&self) -> &'static str {
        "cloudflared"
    }
    fn name(&self) -> &'static str {
        "Cloudflare Tunnel (cloudflared)"
    }
    fn binary(&self) -> &'static str {
        "cloudflared"
    }
    fn docs_url(&self) -> &'static str {
        "https://developers.cloudflare.com/tunnel/downloads/"
    }
    fn common_locations(&self, _home: &std::path::Path) -> Vec<PathBuf> {
        let mut v = vec![];
        if cfg!(windows) {
            v.push(PathBuf::from(r"C:\Program Files (x86)\cloudflared\cloudflared.exe"));
            v.push(PathBuf::from(r"C:\Program Files\cloudflared\cloudflared.exe"));
        }
        v.extend(brew_bins("cloudflared"));
        v
    }
    fn install_methods(&self, os: Os) -> Vec<InstallSpec> {
        let src = "https://developers.cloudflare.com/tunnel/downloads/";
        match os {
            Os::Windows => vec![InstallSpec {
                id: "github-release",
                label: "Tải bản phát hành chính thức từ GitHub (xác minh SHA-256 + chữ ký Authenticode, không cần quyền Admin)",
                kind: InstallKind::VerifiedDownload,
                source_url: "https://github.com/cloudflare/cloudflared/releases/latest",
                requires_elevation: false,
            }],
            Os::MacOs => vec![InstallSpec {
                id: "brew",
                label: "Homebrew (cloudflared)",
                kind: InstallKind::Shell { command: "brew install cloudflared".into(), requires: Some("brew") },
                source_url: src,
                requires_elevation: false,
            }],
            Os::Other => vec![],
        }
    }
    fn interpret_auth(&self, _exit_ok: bool, _stdout: &str) -> (AuthState, Option<String>) {
        (AuthState::NotApplicable, Some("Tunnel dùng token (remotely-managed) không cần đăng nhập; Quick Tunnel không cần tài khoản".into()))
    }
}

pub fn all() -> Vec<Box<dyn CliProvider>> {
    vec![Box::new(Antigravity), Box::new(Codex), Box::new(ClaudeCode), Box::new(Cloudflared)]
}

pub fn by_id(id: &str) -> Option<Box<dyn CliProvider>> {
    all().into_iter().find(|p| p.id() == id)
}

pub fn parse_version(out: &str) -> Option<String> {
    let re = regex::Regex::new(r"\d+\.\d+(?:\.\d+)?(?:[-+][0-9A-Za-z.\-]+)?").ok()?;
    re.find(out).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing_real_formats() {
        assert_eq!(parse_version("2.1.317 (Claude Code)").as_deref(), Some("2.1.317"));
        assert_eq!(parse_version("codex-cli 0.166.0").as_deref(), Some("0.166.0"));
        assert_eq!(parse_version("cloudflared version 2026.10.7 (built 2026-10-30T09:43 UTC)").as_deref(), Some("2026.10.7"));
        assert_eq!(parse_version("no digits"), None);
    }

    #[test]
    fn claude_auth_json_contract() {
        let out = r#"{"loggedIn": false, "authMethod": "none"}"#;
        assert_eq!(ClaudeCode.interpret_auth(false, out).0, AuthState::LoggedOut);
        let out = r#"{"loggedIn": true, "authMethod": "claude.ai"}"#;
        let (s, d) = ClaudeCode.interpret_auth(true, out);
        assert_eq!(s, AuthState::LoggedIn);
        assert!(d.unwrap().contains("claude.ai"));
    }

    #[test]
    fn codex_auth_conservative() {
        assert_eq!(Codex.interpret_auth(true, "Logged in using ChatGPT").0, AuthState::LoggedIn);
        assert_eq!(Codex.interpret_auth(false, "Not logged in").0, AuthState::LoggedOut);
        assert_eq!(Codex.interpret_auth(true, "???").0, AuthState::Unknown);
    }

    #[test]
    fn every_provider_has_install_methods_on_both_os() {
        for p in all() {
            assert!(!p.install_methods(Os::Windows).is_empty(), "{}", p.id());
            assert!(!p.install_methods(Os::MacOs).is_empty(), "{}", p.id());
            for m in p.install_methods(Os::Windows).into_iter().chain(p.install_methods(Os::MacOs)) {
                assert!(m.source_url.starts_with("https://"));
                if let InstallKind::Shell { command, .. } = m.kind {
                    assert!(!command.contains("sudo"), "không yêu cầu sudo ngầm");
                }
            }
        }
    }

    #[test]
    fn diagnostics_are_fixed_allowlists() {
        let d = ClaudeCode.diagnostic_commands();
        assert!(d.contains(&(&["--version"] as &[&str])));
        assert!(d.contains(&(&["auth", "status"] as &[&str])));
    }
}

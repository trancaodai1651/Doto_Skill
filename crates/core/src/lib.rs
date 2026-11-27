//! doto-core: nền tảng dùng chung cho mọi crate của Doto Skill.
//!
//! - [`paths`]: vị trí dữ liệu ứng dụng theo OS.
//! - [`db`]: SQLite + migrations + backup/restore.
//! - [`settings`]: cấu hình ứng dụng dạng key/value JSON có kiểu.
//! - [`audit`]: nhật ký kiểm toán.
//! - [`secrets`]: kho bí mật OS (Keychain / Credential Manager) — không bao giờ lưu token vào SQLite.

pub mod audit;
pub mod db;
pub mod events;
pub mod jobs;
pub mod paths;
pub mod process;
pub mod secrets;
pub mod settings;

pub use db::Db;

/// Thời điểm hiện tại dạng RFC3339 UTC (định dạng thống nhất trong DB/API).
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Sinh id ngẫu nhiên (UUID v4) cho tài nguyên — không đoán được, chống IDOR liệt kê.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Sinh token ngẫu nhiên an toàn (base64url, `bytes` byte entropy).
pub fn random_token(bytes: usize) -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// SHA-256 hex của chuỗi (dùng để lưu hash token phiên, mã ghép nối…).
pub fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(input.as_ref()))
}

/// Che bí mật trong chuỗi log: token GitHub, Cloudflare tunnel token, Bearer…
pub fn redact_secrets(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for word in input.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '=') {
        let trimmed = word.trim_end_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '=');
        let suffix = &word[trimmed.len()..];
        let lower = trimmed.to_ascii_lowercase();
        let secret = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"].iter().any(|p| lower.starts_with(p))
            || (trimmed.len() >= 80 && trimmed.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-.".contains(c)));
        if secret {
            out.push_str("[REDACTED]");
        } else {
            out.push_str(trimmed);
        }
        out.push_str(suffix);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_github_tokens_and_long_blobs() {
        let s = redact_secrets("token=ghp_abcdefghijklmnop and Bearer gho_XYZ123 ok");
        assert!(!s.contains("ghp_abc"));
        assert!(!s.contains("gho_XYZ"));
        assert!(s.contains("ok"));
        let blob = "a".repeat(120);
        assert_eq!(redact_secrets(&format!("run --token {blob}")), "run --token [REDACTED]");
    }

    #[test]
    fn tokens_are_unique_and_long() {
        let a = random_token(32);
        let b = random_token(32);
        assert_ne!(a, b);
        assert!(a.len() >= 42);
    }
}

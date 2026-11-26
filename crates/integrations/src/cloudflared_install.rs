//! Cài cloudflared trên Windows từ GitHub Releases chính thức (cloudflare/cloudflared):
//! xác minh SHA-256 (trường `digest` của asset hoặc bảng checksum trong release notes) và
//! chữ ký Authenticode (Status=Valid, publisher Cloudflare) trước khi đưa vào `<data>/bin`.

use std::path::Path;

use sha2::Digest;

use doto_core::{jobs, Db};

const RELEASE_API: &str = "https://api.github.com/repos/cloudflare/cloudflared/releases/latest";

pub fn asset_name() -> &'static str {
    // Windows ARM64 chạy bản amd64 qua giả lập; cloudflared không phát hành bản windows-arm64.
    "cloudflared-windows-amd64.exe"
}

/// Tìm checksum cho asset: ưu tiên `digest: "sha256:..."`, sau đó quét release notes.
pub fn expected_sha256(release: &serde_json::Value, asset: &str) -> Option<String> {
    if let Some(a) = release["assets"].as_array().and_then(|v| v.iter().find(|a| a["name"].as_str() == Some(asset))) {
        if let Some(d) = a["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")) {
            if d.len() == 64 {
                return Some(d.to_ascii_lowercase());
            }
        }
    }
    let body = release["body"].as_str()?;
    let re = regex::Regex::new(&format!(r"(?m){}\s*[:|]?\s*`?([A-Fa-f0-9]{{64}})", regex::escape(asset))).ok()?;
    re.captures(body).map(|c| c[1].to_ascii_lowercase())
}

pub async fn install_windows(db: &Db, job_id: &str, bin_dir: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(cfg!(windows), "phương thức này chỉ dành cho Windows");
    let http = reqwest::Client::builder().user_agent(concat!("DotoSkill/", env!("CARGO_PKG_VERSION"))).build()?;
    jobs::append_log(db, job_id, &format!("Đọc bản phát hành mới nhất: {RELEASE_API}")).await?;
    let rel: serde_json::Value = http.get(RELEASE_API).send().await?.error_for_status()?.json().await?;
    let tag = rel["tag_name"].as_str().unwrap_or("?").to_string();
    let asset = asset_name();
    let url = rel["assets"]
        .as_array()
        .and_then(|v| v.iter().find(|a| a["name"].as_str() == Some(asset)))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| anyhow::anyhow!("không thấy asset {asset} trong release {tag}"))?
        .to_string();
    anyhow::ensure!(url.starts_with("https://github.com/cloudflare/cloudflared/releases/download/"), "URL tải không thuộc repo chính thức");
    let expected = expected_sha256(&rel, asset);
    jobs::append_log(db, job_id, &format!("Phiên bản {tag}; tải {url}")).await?;
    let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?;
    anyhow::ensure!(bytes.len() > 1_000_000 && bytes.len() < 300_000_000, "kích thước file bất thường");
    let actual = hex::encode(sha2::Sha256::digest(&bytes));
    jobs::append_log(db, job_id, &format!("SHA-256: {actual}")).await?;
    match &expected {
        Some(e) if e == &actual => jobs::append_log(db, job_id, "✔ SHA-256 khớp checksum công bố").await?,
        Some(e) => anyhow::bail!("SHA-256 KHÔNG khớp (mong đợi {e}) — đã huỷ cài"),
        None => jobs::append_log(db, job_id, "⚠ release không công bố checksum cho asset — dựa vào chữ ký Authenticode").await?,
    }
    tokio::fs::create_dir_all(bin_dir).await?;
    let tmp = bin_dir.join(".cloudflared.download.exe");
    tokio::fs::write(&tmp, &bytes).await?;
    // Xác minh chữ ký số Authenticode qua PowerShell (đường dẫn truyền bằng biến môi trường, không nội suy chuỗi)
    let mut cmd = crate::manager::command(Path::new("powershell.exe"));
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "$s = Get-AuthenticodeSignature -LiteralPath $env:DOTO_VERIFY_PATH; \"$($s.Status)|$($s.SignerCertificate.Subject)\"",
    ])
    .env("DOTO_VERIFY_PATH", &tmp);
    let out = cmd.output().await?;
    let sig = String::from_utf8_lossy(&out.stdout).trim().to_string();
    jobs::append_log(db, job_id, &format!("Chữ ký: {sig}")).await?;
    let valid = sig.starts_with("Valid|") && sig.to_ascii_lowercase().contains("cloudflare");
    if !valid {
        let _ = tokio::fs::remove_file(&tmp).await;
        anyhow::bail!("chữ ký Authenticode không hợp lệ hoặc không phải Cloudflare — đã huỷ cài");
    }
    let dest = bin_dir.join("cloudflared.exe");
    let _ = tokio::fs::remove_file(&dest).await;
    tokio::fs::rename(&tmp, &dest).await?;
    jobs::append_log(db, job_id, &format!("✔ Đã cài {}", dest.display())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_from_digest_or_body() {
        let sha = "a".repeat(64);
        let r = serde_json::json!({"assets":[{"name":"cloudflared-windows-amd64.exe","digest":format!("sha256:{sha}")}]});
        assert_eq!(expected_sha256(&r, "cloudflared-windows-amd64.exe"), Some(sha.clone()));
        let body =
            format!("### SHA256 Checksums:\n```\ncloudflared-darwin-amd64.tgz: {}\ncloudflared-windows-amd64.exe: {}\n```", "b".repeat(64), "C".repeat(64));
        let r = serde_json::json!({"assets":[], "body": body});
        assert_eq!(expected_sha256(&r, "cloudflared-windows-amd64.exe"), Some("c".repeat(64)));
        assert_eq!(expected_sha256(&serde_json::json!({"assets":[]}), "x"), None);
    }
}

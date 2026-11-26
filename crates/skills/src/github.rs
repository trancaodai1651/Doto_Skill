//! GitHub REST client tối giản: resolve ref → commit SHA, tải tarball read-only, so sánh commit,
//! OAuth Device Flow. Token chỉ nằm trong header Authorization — không bao giờ trong URL hay log.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

pub const MAX_TARBALL_BYTES: u64 = 200 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: u64 = 500 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 50_000;
const UA: &str = concat!("DotoSkill/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("không tìm thấy repo hoặc ref (repo private cần đăng nhập GitHub có quyền đọc)")]
    NotFound,
    #[error("GitHub từ chối truy cập (token hết hạn/thiếu quyền hoặc vượt giới hạn API)")]
    Unauthorized,
    #[error("lỗi mạng khi kết nối GitHub: {0}")]
    Network(String),
    #[error("{0}")]
    Invalid(String),
}

impl From<reqwest::Error> for GithubError {
    fn from(e: reqwest::Error) -> Self {
        // reqwest::Error có thể chứa URL — URL của chúng ta không bao giờ chứa token.
        GithubError::Network(e.without_url().to_string())
    }
}

#[derive(Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    token: Option<String>,
    api_base: String,
    web_base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoInfo {
    pub private: bool,
    pub default_branch: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    Token(String),
    Expired,
    Denied,
}

impl GithubClient {
    pub fn new(token: Option<String>) -> Self {
        Self::with_bases(token, "https://api.github.com", "https://github.com")
    }

    pub fn with_bases(token: Option<String>, api_base: &str, web_base: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(UA)
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("reqwest client");
        Self { http, token, api_base: api_base.trim_end_matches('/').into(), web_base: web_base.trim_end_matches('/').into() }
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        let mut rb = self.http.get(format!("{}{}", self.api_base, path)).header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(t) = &self.token {
            rb = rb.bearer_auth(t);
        }
        rb
    }

    fn check(status: reqwest::StatusCode) -> Result<(), GithubError> {
        match status.as_u16() {
            200..=299 => Ok(()),
            401 | 403 | 429 => Err(GithubError::Unauthorized),
            404 | 422 => Err(GithubError::NotFound),
            s => Err(GithubError::Network(format!("HTTP {s}"))),
        }
    }

    pub async fn repo(&self, owner: &str, repo: &str) -> Result<RepoInfo, GithubError> {
        let r = self.get(&format!("/repos/{owner}/{repo}")).header("Accept", "application/vnd.github+json").send().await?;
        Self::check(r.status())?;
        Ok(r.json().await?)
    }

    /// Ref → SHA commit đầy đủ. `None` ⇒ nhánh mặc định.
    pub async fn resolve_commit(&self, owner: &str, repo: &str, git_ref: Option<&str>) -> Result<String, GithubError> {
        let r = match git_ref {
            Some(r) => r.to_string(),
            None => self.repo(owner, repo).await?.default_branch,
        };
        let resp = self.get(&format!("/repos/{owner}/{repo}/commits/{}", urlencode_ref(&r))).header("Accept", "application/vnd.github.sha").send().await?;
        Self::check(resp.status())?;
        let sha = resp.text().await?.trim().to_string();
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(GithubError::Invalid("GitHub trả SHA không hợp lệ".into()));
        }
        Ok(sha)
    }

    /// Tải tarball của commit và giải nén an toàn vào `dest` (chỉ phần `sub_path` nếu có).
    pub async fn download_and_extract(&self, owner: &str, repo: &str, sha: &str, sub_path: Option<&str>, dest: &Path) -> Result<ExtractReport, GithubError> {
        let resp = self.get(&format!("/repos/{owner}/{repo}/tarball/{sha}")).send().await?;
        Self::check(resp.status())?;
        if resp.content_length().unwrap_or(0) > MAX_TARBALL_BYTES {
            return Err(GithubError::Invalid("repo quá lớn để import (> 200 MB)".into()));
        }
        let tmp_tar = dest.with_extension("download.tar.gz");
        if let Some(parent) = tmp_tar.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| GithubError::Invalid(e.to_string()))?;
        }
        let mut f = tokio::fs::File::create(&tmp_tar).await.map_err(|e| GithubError::Invalid(e.to_string()))?;
        let mut total = 0u64;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            total += chunk.len() as u64;
            if total > MAX_TARBALL_BYTES {
                drop(f);
                let _ = tokio::fs::remove_file(&tmp_tar).await;
                return Err(GithubError::Invalid("repo quá lớn để import (> 200 MB)".into()));
            }
            f.write_all(&chunk).await.map_err(|e| GithubError::Invalid(e.to_string()))?;
        }
        f.flush().await.map_err(|e| GithubError::Invalid(e.to_string()))?;
        drop(f);
        let (tar, d, sp) = (tmp_tar.clone(), dest.to_path_buf(), sub_path.map(String::from));
        let res = tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&tar)?;
            extract_tar_gz(file, &d, sp.as_deref())
        })
        .await
        .map_err(|e| GithubError::Invalid(e.to_string()))?;
        let _ = tokio::fs::remove_file(&tmp_tar).await;
        res.map_err(|e| GithubError::Invalid(e.to_string()))
    }

    pub async fn compare(&self, owner: &str, repo: &str, base: &str, head: &str) -> Result<(Vec<ChangedFile>, String), GithubError> {
        let resp = self.get(&format!("/repos/{owner}/{repo}/compare/{base}...{head}")).header("Accept", "application/vnd.github+json").send().await?;
        Self::check(resp.status())?;
        let v: serde_json::Value = resp.json().await?;
        let files = v["files"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(300)
            .map(|f| ChangedFile {
                filename: f["filename"].as_str().unwrap_or("").into(),
                status: f["status"].as_str().unwrap_or("").into(),
                additions: f["additions"].as_i64().unwrap_or(0),
                deletions: f["deletions"].as_i64().unwrap_or(0),
            })
            .collect();
        Ok((files, v["html_url"].as_str().unwrap_or("").to_string()))
    }

    /// Thông tin tài khoản + scopes (header X-OAuth-Scopes).
    pub async fn user(&self) -> Result<(String, Vec<String>), GithubError> {
        let resp = self.get("/user").header("Accept", "application/vnd.github+json").send().await?;
        Self::check(resp.status())?;
        let scopes = resp
            .headers()
            .get("x-oauth-scopes")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        let v: serde_json::Value = resp.json().await?;
        Ok((v["login"].as_str().unwrap_or("").to_string(), scopes))
    }

    pub async fn device_start(&self, client_id: &str) -> Result<DeviceCode, GithubError> {
        let resp = self
            .http
            .post(format!("{}/login/device/code", self.web_base))
            .header("Accept", "application/json")
            .form(&[("client_id", client_id), ("scope", "repo read:user")])
            .send()
            .await?;
        Self::check(resp.status())?;
        let v: serde_json::Value = resp.json().await?;
        if let Some(e) = v["error"].as_str() {
            return Err(GithubError::Invalid(format!("GitHub Device Flow: {e}")));
        }
        serde_json::from_value(v).map_err(|e| GithubError::Invalid(e.to_string()))
    }

    pub async fn device_poll(&self, client_id: &str, device_code: &str) -> Result<DevicePoll, GithubError> {
        let resp = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web_base))
            .header("Accept", "application/json")
            .form(&[("client_id", client_id), ("device_code", device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")])
            .send()
            .await?;
        Self::check(resp.status())?;
        let v: serde_json::Value = resp.json().await?;
        if let Some(t) = v["access_token"].as_str() {
            return Ok(DevicePoll::Token(t.to_string()));
        }
        Ok(match v["error"].as_str().unwrap_or("") {
            "authorization_pending" => DevicePoll::Pending,
            "slow_down" => DevicePoll::SlowDown,
            "expired_token" => DevicePoll::Expired,
            "access_denied" => DevicePoll::Denied,
            other => return Err(GithubError::Invalid(format!("GitHub Device Flow: {other}"))),
        })
    }
}

fn urlencode_ref(r: &str) -> String {
    r.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.~".contains(c) { c.to_string() } else { format!("%{:02X}", c as u32) }).collect()
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReport {
    pub files: usize,
    pub bytes: u64,
    pub skipped: Vec<String>,
}

/// Giải nén tar.gz an toàn: bỏ thư mục gốc GitHub, chặn path traversal/absolute, bỏ symlink/hardlink,
/// giới hạn số entry và tổng dung lượng giải nén (chống zip/tar bomb).
pub fn extract_tar_gz<R: Read>(reader: R, dest: &Path, sub_path: Option<&str>) -> std::io::Result<ExtractReport> {
    let gz = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(gz);
    std::fs::create_dir_all(dest)?;
    let mut report = ExtractReport::default();
    let sub: Option<PathBuf> = sub_path.map(|s| s.split('/').filter(|x| !x.is_empty()).collect());
    for (n, entry) in archive.entries()?.enumerate() {
        if n >= MAX_ENTRIES {
            return Err(std::io::Error::other("archive có quá nhiều file"));
        }
        let mut entry = entry?;
        let raw = entry.path()?.into_owned();
        // bỏ component đầu (owner-repo-sha/)
        let mut comps = raw.components();
        comps.next();
        let rel: PathBuf = comps.as_path().to_path_buf();
        if rel.as_os_str().is_empty() {
            continue;
        }
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            report.skipped.push(format!("đường dẫn không an toàn: {}", raw.display()));
            continue;
        }
        let rel = match &sub {
            Some(s) => match rel.strip_prefix(s) {
                Ok(r) => r.to_path_buf(),
                Err(_) => continue,
            },
            None => rel,
        };
        let et = entry.header().entry_type();
        if et.is_symlink() || et.is_hard_link() {
            report.skipped.push(format!("bỏ qua link: {}", rel.display()));
            continue;
        }
        let out = dest.join(&rel);
        if et.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if !et.is_file() {
            continue;
        }
        let size = entry.header().size()?;
        report.bytes += size;
        if report.bytes > MAX_EXTRACTED_BYTES {
            return Err(std::io::Error::other("dung lượng giải nén vượt giới hạn an toàn (tar bomb?)"));
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        let copied = std::io::copy(&mut (&mut entry).take(size + 1), &mut f)?;
        if copied > size {
            return Err(std::io::Error::other("kích thước entry không khớp header"));
        }
        report.files += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_tar(entries: &[(&str, &[u8])], symlink: Option<(&str, &str)>) -> Vec<u8> {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (p, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            // ghi path thô (kể cả "..") bằng cách set trực tiếp bytes tên
            let name = p.as_bytes();
            h.as_old_mut().name[..name.len()].copy_from_slice(name);
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        if let Some((p, target)) = symlink {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_size(0);
            b.append_link(&mut h, p, target).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extracts_and_strips_root_and_filters_subpath() {
        let data = build_tar(&[("o-r-abc/skills/a/SKILL.md", b"---\nname: a\n---"), ("o-r-abc/README.md", b"hi"), ("o-r-abc/skills/b/SKILL.md", b"b")], None);
        let d = tempfile::tempdir().unwrap();
        let r = extract_tar_gz(&data[..], d.path(), Some("skills")).unwrap();
        assert_eq!(r.files, 2);
        assert!(d.path().join("a/SKILL.md").is_file());
        assert!(!d.path().join("README.md").exists());
    }

    #[test]
    fn blocks_traversal_and_symlinks() {
        let data = build_tar(&[("o-r-abc/../../evil.txt", b"x"), ("o-r-abc/ok.txt", b"ok")], Some(("o-r-abc/link", "/etc/passwd")));
        let d = tempfile::tempdir().unwrap();
        let inner = d.path().join("dest");
        let r = extract_tar_gz(&data[..], &inner, None).unwrap();
        assert_eq!(r.files, 1);
        assert!(!d.path().join("evil.txt").exists());
        assert!(!inner.join("link").exists());
        assert_eq!(r.skipped.len(), 2, "{:?}", r.skipped);
    }

    #[test]
    fn ref_encoding() {
        assert_eq!(urlencode_ref("feature/x"), "feature%2Fx");
        assert_eq!(urlencode_ref("v1.0"), "v1.0");
    }
}

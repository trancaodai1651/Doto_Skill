//! Ánh xạ skill đã duyệt sang thư mục skills mà từng công cụ AI đọc (theo tài liệu chính thức, 2026-11):
//! - Claude Code: `~/.claude/skills/<name>/SKILL.md` (frontmatter tuỳ chọn; name mặc định = tên thư mục)
//!   — https://code.claude.com/docs/en/skills
//! - OpenAI Codex: `~/.agents/skills/<name>/SKILL.md` (bắt buộc `name` + `description`)
//!   — https://learn.chatgpt.com/docs/build-skills
//! - Google Antigravity (App + CLI + IDE): `~/.gemini/config/skills/<name>/` (bắt buộc `description`)
//!   — https://antigravity.google/docs/skills
//!
//! Cài đặt = SAO CHÉP thư mục (không symlink/junction), kèm file đánh dấu `.doto-skill.json` để gỡ
//! chỉ những gì Doto Skill đã tạo. Không bao giờ ghi đè thư mục người dùng tự tạo.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MARKER: &str = ".doto-skill.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Antigravity,
}

impl Provider {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "antigravity" => Some(Self::Antigravity),
            _ => None,
        }
    }
    pub fn id(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
        }
    }
    /// Thư mục skills cấp người dùng.
    pub fn user_skills_dir(&self) -> Option<PathBuf> {
        let home = home_dir()?;
        Some(match self {
            Self::Claude => home.join(".claude").join("skills"),
            Self::Codex => home.join(".agents").join("skills"),
            Self::Antigravity => home.join(".gemini").join("config").join("skills"),
        })
    }
    /// Kiểm tra skill có đáp ứng yêu cầu frontmatter của provider không.
    pub fn validate(&self, meta: &serde_json::Value) -> Result<(), String> {
        let has = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(|s| !s.trim().is_empty()).unwrap_or(false);
        match self {
            Self::Claude => Ok(()),
            Self::Codex if !(has("name") && has("description")) => Err("Codex yêu cầu frontmatter có cả `name` và `description`".into()),
            Self::Antigravity if !has("description") => Err("Antigravity yêu cầu frontmatter có `description`".into()),
            _ => Ok(()),
        }
    }
}

/// Thư mục home; `DOTO_PROVIDER_HOME` cho phép test cô lập.
pub fn home_dir() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("DOTO_PROVIDER_HOME") {
        return Some(PathBuf::from(h));
    }
    doto_core::paths::user_home_dir()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub skill_id: String,
    pub source_id: String,
    pub revision: String,
    pub installed_at: String,
}

/// Sao chép thư mục skill (bỏ symlink, bỏ `.git`) vào `<provider_dir>/<folder_name>`.
pub fn install_copy(src_dir: &Path, provider_dir: &Path, folder_name: &str, marker: &Marker) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !folder_name.is_empty() && folder_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "tên thư mục skill không hợp lệ"
    );
    let target = provider_dir.join(folder_name);
    if target.exists() {
        let m = target.join(MARKER);
        anyhow::ensure!(m.is_file(), "{} đã tồn tại và không do Doto Skill tạo — không ghi đè", target.display());
        std::fs::remove_dir_all(&target)?;
    }
    let staging = provider_dir.join(format!(".{folder_name}.doto-staging"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let res = (|| -> anyhow::Result<()> {
        for e in walkdir::WalkDir::new(src_dir).follow_links(false).into_iter().filter_entry(|e| e.file_name() != ".git") {
            let e = e?;
            let rel = e.path().strip_prefix(src_dir)?;
            if rel.as_os_str().is_empty() || e.file_type().is_symlink() || rel.file_name().map(|n| n == MARKER).unwrap_or(false) {
                continue;
            }
            let out = staging.join(rel);
            if e.file_type().is_dir() {
                std::fs::create_dir_all(&out)?;
            } else if e.file_type().is_file() {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(e.path(), &out)?;
            }
        }
        std::fs::write(staging.join(MARKER), serde_json::to_vec_pretty(marker)?)?;
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    std::fs::rename(&staging, &target)?;
    Ok(target)
}

/// Gỡ — chỉ xoá khi có marker của Doto Skill khớp skill_id.
pub fn uninstall(target: &Path, skill_id: &str) -> anyhow::Result<()> {
    if !target.exists() {
        return Ok(());
    }
    let m: Marker = serde_json::from_slice(&std::fs::read(target.join(MARKER)).map_err(|_| anyhow::anyhow!("thư mục không có dấu Doto Skill — không xoá"))?)?;
    anyhow::ensure!(m.skill_id == skill_id, "thư mục thuộc skill khác — không xoá");
    std::fs::remove_dir_all(target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker() -> Marker {
        Marker { skill_id: "s1".into(), source_id: "src".into(), revision: "r".into(), installed_at: "t".into() }
    }

    #[test]
    fn provider_requirements() {
        let full = serde_json::json!({"name":"a","description":"b"});
        let desc_only = serde_json::json!({"description":"b"});
        let none = serde_json::json!({});
        assert!(Provider::Claude.validate(&none).is_ok());
        assert!(Provider::Codex.validate(&desc_only).is_err());
        assert!(Provider::Codex.validate(&full).is_ok());
        assert!(Provider::Antigravity.validate(&desc_only).is_ok());
        assert!(Provider::Antigravity.validate(&none).is_err());
    }

    #[test]
    fn install_copy_and_safe_uninstall() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("SKILL.md"), "---\nname: a\n---").unwrap();
        std::fs::create_dir_all(src.path().join("ref")).unwrap();
        std::fs::write(src.path().join("ref/x.md"), "x").unwrap();
        let home = tempfile::tempdir().unwrap();
        let t = install_copy(src.path(), home.path(), "my-skill", &marker()).unwrap();
        assert!(t.join("SKILL.md").is_file() && t.join("ref/x.md").is_file() && t.join(MARKER).is_file());
        // cài lại (cập nhật) được vì có marker
        install_copy(src.path(), home.path(), "my-skill", &marker()).unwrap();
        assert!(uninstall(&t, "other").is_err());
        uninstall(&t, "s1").unwrap();
        assert!(!t.exists());
    }

    #[test]
    fn never_overwrites_user_folder() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("SKILL.md"), "x").unwrap();
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("mine")).unwrap();
        std::fs::write(home.path().join("mine/SKILL.md"), "user content").unwrap();
        assert!(install_copy(src.path(), home.path(), "mine", &marker()).is_err());
        assert_eq!(std::fs::read_to_string(home.path().join("mine/SKILL.md")).unwrap(), "user content");
        assert!(uninstall(&home.path().join("mine"), "s1").is_err());
        assert!(install_copy(src.path(), home.path(), "../escape", &marker()).is_err());
    }
}

//! Trung tâm Bộ nhớ chung (Shared Memory) & Bàn giao 3 AI (Google Antigravity · Claude Code · OpenAI Codex).
//!
//! Chỉ cho phép đọc/ghi trên **allowlist cố định** các tệp Markdown điều phối trong thư mục workspace được cấu hình
//! (mặc định tự nhận diện gốc repository nếu có `docs/shared/PROJECT_MEMORY.md`, hoặc `<data>/workspace-memory`).
//! Mọi nội dung ghi xuống đĩa đều bắt buộc đi qua `doto_core::redact_secrets` để không bao giờ lưu lộ token/mật khẩu.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const MAX_DOC_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedDocSpec {
    pub id: &'static str,
    pub rel_path: &'static str,
    pub group: &'static str,
    pub title_vi: &'static str,
    pub title_en: &'static str,
    pub agents: &'static [&'static str],
}

pub const MANAGED_DOCS: &[ManagedDocSpec] = &[
    ManagedDocSpec {
        id: "PROJECT_MEMORY.md",
        rel_path: "docs/shared/PROJECT_MEMORY.md",
        group: "shared",
        title_vi: "Bộ nhớ dự án dùng chung (Kiến trúc, quy tắc, bẫy kỹ thuật)",
        title_en: "Shared project memory (Architecture, rules & technical pitfalls)",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "ROADMAP.md",
        rel_path: "docs/shared/ROADMAP.md",
        group: "shared",
        title_vi: "Lộ trình triển khai & trạng thái từng Phase",
        title_en: "Implementation roadmap & phase status",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "TASK_BOARD.md",
        rel_path: "docs/shared/TASK_BOARD.md",
        group: "shared",
        title_vi: "Bảng công việc In-progress / Pending / Blocked / Done",
        title_en: "Shared task board (In-progress / Pending / Blocked / Done)",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "DECISIONS.md",
        rel_path: "docs/shared/DECISIONS.md",
        group: "shared",
        title_vi: "Sổ quyết định kiến trúc (ADR)",
        title_en: "Architecture Decision Records (ADR)",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "LAST_TEST_RUN.md",
        rel_path: "docs/shared/LAST_TEST_RUN.md",
        group: "shared",
        title_vi: "Bằng chứng chạy kiểm thử gần nhất",
        title_en: "Latest verified test run log",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "HANDOFF_ANTIGRAVITY.md",
        rel_path: "docs/agents/HANDOFF_ANTIGRAVITY.md",
        group: "handoff",
        title_vi: "Bàn giao phiên Google Antigravity",
        title_en: "Google Antigravity handoff",
        agents: &["antigravity"],
    },
    ManagedDocSpec {
        id: "HANDOFF_CLAUDE.md",
        rel_path: "docs/agents/HANDOFF_CLAUDE.md",
        group: "handoff",
        title_vi: "Bàn giao phiên Claude Code",
        title_en: "Claude Code handoff",
        agents: &["claude"],
    },
    ManagedDocSpec {
        id: "HANDOFF_CODEX.md",
        rel_path: "docs/agents/HANDOFF_CODEX.md",
        group: "handoff",
        title_vi: "Bàn giao phiên OpenAI Codex",
        title_en: "OpenAI Codex handoff",
        agents: &["codex"],
    },
    ManagedDocSpec {
        id: "MEMORY.md",
        rel_path: "MEMORY.md",
        group: "workspace",
        title_vi: "Bộ nhớ nhanh cấp thư mục làm việc",
        title_en: "Workspace quick memory",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "HANDOFF.md",
        rel_path: "HANDOFF.md",
        group: "workspace",
        title_vi: "Sổ bàn giao nhanh liên phiên",
        title_en: "Quick cross-session handoff",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "AGENTS.md",
        rel_path: "AGENTS.md",
        group: "workspace",
        title_vi: "Quy tắc điều phối chung (Codex & Antigravity tự nạp)",
        title_en: "Universal agent rules (Codex & Antigravity)",
        agents: &["antigravity", "claude", "codex"],
    },
    ManagedDocSpec {
        id: "CLAUDE.md",
        rel_path: "CLAUDE.md",
        group: "workspace",
        title_vi: "Điểm neo ngữ cảnh cho Claude Code",
        title_en: "Context anchor for Claude Code",
        agents: &["claude"],
    },
    ManagedDocSpec {
        id: "GEMINI.md",
        rel_path: "GEMINI.md",
        group: "workspace",
        title_vi: "Điểm neo ngữ cảnh cho Google Antigravity",
        title_en: "Context anchor for Google Antigravity",
        agents: &["antigravity"],
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDocStatus {
    pub id: &'static str,
    pub rel_path: &'static str,
    pub group: &'static str,
    pub title_vi: &'static str,
    pub title_en: &'static str,
    pub agents: &'static [&'static str],
    pub exists: bool,
    pub size_bytes: u64,
    pub updated_at: Option<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffEntry {
    pub id: i64,
    pub agent: String,
    pub repo_sha: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub workspace_dir: String,
    pub is_custom_dir: bool,
    pub docs: Vec<MemoryDocStatus>,
    pub recent_handoffs: Vec<HandoffEntry>,
}

pub fn find_spec(doc_id: &str) -> Option<&'static ManagedDocSpec> {
    MANAGED_DOCS.iter().find(|d| d.id == doc_id || d.rel_path == doc_id)
}

/// Xác định thư mục gốc cho bộ nhớ chung: ưu tiên cấu hình người dùng → thư mục repo hiện tại (nếu có `docs/shared/PROJECT_MEMORY.md`) → `<data>/workspace-memory`.
pub fn resolve_workspace_dir(configured: Option<&str>, fallback_data_dir: &Path) -> (PathBuf, bool) {
    if let Some(c) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        return (PathBuf::from(c), true);
    }
    if let Ok(cwd) = std::env::current_dir() {
        for dir in [Some(cwd.as_path()), cwd.parent(), cwd.parent().and_then(|p| p.parent())].into_iter().flatten() {
            if dir.join("docs").join("shared").join("PROJECT_MEMORY.md").is_file() {
                return (dir.to_path_buf(), false);
            }
        }
    }
    (fallback_data_dir.to_path_buf(), false)
}

fn safe_doc_path(root: &Path, spec: &ManagedDocSpec) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("không tạo được thư mục bộ nhớ: {e}"))?;
    let p = root.join(spec.rel_path);
    if let Ok(meta) = std::fs::symlink_metadata(&p) {
        if meta.file_type().is_symlink() {
            return Err("từ chối đọc/ghi qua symlink".into());
        }
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("không tạo được thư mục con: {e}"))?;
        if !doto_core::paths::is_within(root, parent) {
            return Err("đường dẫn thoát khỏi thư mục bộ nhớ".into());
        }
    }
    Ok(p)
}

pub fn scan_docs(root: &Path) -> Vec<MemoryDocStatus> {
    MANAGED_DOCS
        .iter()
        .map(|spec| {
            let full = root.join(spec.rel_path);
            let mut exists = false;
            let mut size_bytes = 0;
            let mut updated_at = None;
            let mut preview = String::new();
            if let Ok(meta) = std::fs::symlink_metadata(&full) {
                if meta.is_file() && !meta.file_type().is_symlink() {
                    exists = true;
                    size_bytes = meta.len();
                    updated_at = meta.modified().ok().map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
                    if let Ok(raw) = std::fs::read_to_string(&full) {
                        preview = raw.chars().take(280).collect();
                    }
                }
            }
            MemoryDocStatus {
                id: spec.id,
                rel_path: spec.rel_path,
                group: spec.group,
                title_vi: spec.title_vi,
                title_en: spec.title_en,
                agents: spec.agents,
                exists,
                size_bytes,
                updated_at,
                preview,
            }
        })
        .collect()
}

pub fn read_doc(root: &Path, doc_id: &str) -> Result<(&'static ManagedDocSpec, bool, String), String> {
    let spec = find_spec(doc_id).ok_or_else(|| format!("tệp `{doc_id}` không nằm trong danh sách allowlist bộ nhớ chung"))?;
    let p = safe_doc_path(root, spec)?;
    if !p.is_file() {
        return Ok((spec, false, default_template(spec.id)));
    }
    let meta = std::fs::metadata(&p).map_err(|e| e.to_string())?;
    if meta.len() as usize > MAX_DOC_BYTES {
        return Err("tệp vượt quá giới hạn 512 KB".into());
    }
    let content = std::fs::read_to_string(&p).map_err(|e| format!("không đọc được tệp UTF-8: {e}"))?;
    Ok((spec, true, content))
}

pub fn write_doc(root: &Path, doc_id: &str, content: &str) -> Result<&'static ManagedDocSpec, String> {
    if content.len() > MAX_DOC_BYTES {
        return Err("nội dung vượt quá giới hạn 512 KB".into());
    }
    let spec = find_spec(doc_id).ok_or_else(|| format!("tệp `{doc_id}` không nằm trong danh sách allowlist bộ nhớ chung"))?;
    let p = safe_doc_path(root, spec)?;
    let clean = doto_core::redact_secrets(content);
    std::fs::write(&p, clean).map_err(|e| format!("không ghi được tệp: {e}"))?;
    Ok(spec)
}

pub fn append_note(root: &Path, target_id: &str, agent: &str, note: &str) -> Result<&'static ManagedDocSpec, String> {
    let clean_note = doto_core::redact_secrets(note.trim());
    if clean_note.is_empty() || clean_note.chars().count() > 4000 {
        return Err("ghi chú phải từ 1 đến 4000 ký tự".into());
    }
    let spec = find_spec(target_id).ok_or_else(|| "tệp đích không hợp lệ".to_string())?;
    let p = safe_doc_path(root, spec)?;
    let existing = if p.is_file() { std::fs::read_to_string(&p).unwrap_or_default() } else { default_template(spec.id) };
    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    let safe_agent: String = agent.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_' | ' ')).take(32).collect();
    let line = format!("\n- **{ts}** · `{}` — {}\n", if safe_agent.is_empty() { "owner" } else { &safe_agent }, clean_note.replace('\n', " "));
    let next = format!("{}{}", existing.trim_end(), line);
    if next.len() > MAX_DOC_BYTES {
        return Err("tệp bộ nhớ đã đầy (512 KB) — hãy rút gọn trước khi ghi thêm".into());
    }
    std::fs::write(&p, next).map_err(|e| format!("không ghi nối được tệp: {e}"))?;
    Ok(spec)
}

pub fn init_missing_templates(root: &Path, overwrite: bool) -> Result<(Vec<&'static str>, Vec<&'static str>), String> {
    let mut created = vec![];
    let mut kept = vec![];
    for spec in MANAGED_DOCS {
        let p = safe_doc_path(root, spec)?;
        if !p.exists() || overwrite {
            std::fs::write(&p, default_template(spec.id)).map_err(|e| e.to_string())?;
            created.push(spec.id);
        } else {
            kept.push(spec.id);
        }
    }
    Ok((created, kept))
}

pub fn default_template(id: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d");
    match id {
        "CLAUDE.md" => "@AGENTS.md\n@MEMORY.md\n@HANDOFF.md\n".into(),
        "GEMINI.md" => format!(
            "# GEMINI.md — Google Antigravity\n\nĐọc `AGENTS.md`, `MEMORY.md` và `HANDOFF.md` (hoặc `docs/shared/PROJECT_MEMORY.md`) trước khi thực hiện tác vụ. Cập nhật bàn giao sau mỗi mốc ({today}).\n"
        ),
        "AGENTS.md" => format!(
            "# AGENTS.md — Quy chuẩn Điều phối Đa AI (Antigravity · Claude Code · Codex)\n\n1. Đọc `MEMORY.md` / `docs/shared/PROJECT_MEMORY.md` đầu phiên.\n2. Đọc `HANDOFF.md` / `docs/agents/HANDOFF_*.md` để nối tiếp phiên trước.\n3. Không bao giờ ghi token, mật khẩu hay dữ liệu nhạy cảm vào tệp bàn giao ({today}).\n"
        ),
        "HANDOFF.md" => format!(
            "# HANDOFF.md — Sổ bàn giao liên phiên (Antigravity · Claude Code · Codex)\n\n## Nhật ký bàn giao\n- **{today}** · Khởi tạo sổ bàn giao chung trên Doto Skill.\n"
        ),
        _ => format!(
            "# {id}\n\n> Tệp điều phối dùng chung cho Google Antigravity, Claude Code và OpenAI Codex ({today}). Không lưu token/mật khẩu tại đây.\n"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_blocks_arbitrary_files_and_redacts_secrets() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_doc(tmp.path(), "../etc/passwd").is_err());
        assert!(write_doc(tmp.path(), ".env", "SECRET=1").is_err());

        write_doc(tmp.path(), "MEMORY.md", "Quy ước chung: token=ghp_1234567890abcdef").unwrap();
        let (_, exists, content) = read_doc(tmp.path(), "MEMORY.md").unwrap();
        assert!(exists);
        assert!(!content.contains("ghp_123456"));
        assert!(content.contains("[REDACTED]"));

        append_note(tmp.path(), "HANDOFF.md", "antigravity", "Hoàn tất mốc kiểm thử gho_secret999").unwrap();
        let (_, _, h) = read_doc(tmp.path(), "HANDOFF.md").unwrap();
        assert!(!h.contains("gho_secret999"));
        assert!(h.contains("antigravity"));
    }
}

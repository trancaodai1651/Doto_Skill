//! Quét thư mục Skills (input không tin cậy): tìm SKILL.md, parse frontmatter YAML, kiểm tra an toàn.
//! Không bao giờ thực thi script của skill.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const MAX_SKILL_MD_BYTES: u64 = 512 * 1024;
pub const MAX_FILES_PER_SKILL: usize = 2_000;
pub const MAX_SKILLS_PER_SOURCE: usize = 5_000;
const SCRIPT_EXTS: &[&str] = &["sh", "bash", "zsh", "ps1", "psm1", "bat", "cmd", "py", "js", "mjs", "cjs", "ts", "rb", "pl", "exe", "dll", "so", "dylib"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScannedSkill {
    pub canonical_id: String,
    pub name: String,
    pub description: String,
    /// Thư mục skill tương đối so với gốc nguồn ("" = gốc), luôn dùng '/'.
    pub rel_path: String,
    pub revision: String,
    pub metadata: serde_json::Value,
    pub has_scripts: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillFile {
    pub path: String,
    pub size: u64,
    pub executable: bool,
}

/// Tách frontmatter `---\n...\n---` khỏi Markdown.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let c = content.strip_prefix('\u{feff}').unwrap_or(content);
    let Some(rest) = c.strip_prefix("---") else { return (None, c) };
    let rest = rest.trim_start_matches([' ', '\t']);
    let Some(rest) = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")) else { return (None, c) };
    for (idx, _) in rest.match_indices("---") {
        let before_ok = idx == 0 || rest[..idx].ends_with('\n');
        let after = &rest[idx + 3..];
        let after_ok = after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n") || after.trim_start_matches([' ', '\t']).starts_with('\n');
        if before_ok && after_ok {
            return (Some(&rest[..idx]), after.trim_start_matches(['\r', '\n']));
        }
    }
    (None, c)
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in s.trim().chars().flat_map(|c| c.to_lowercase()) {
        let c = fold_vietnamese(ch);
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').chars().take(64).collect::<String>();
    if out.is_empty() {
        "skill".into()
    } else {
        out
    }
}

fn fold_vietnamese(c: char) -> char {
    const MAP: &[(&str, char)] =
        &[("àáạảãâầấậẩẫăằắặẳẵ", 'a'), ("èéẹẻẽêềếệểễ", 'e'), ("ìíịỉĩ", 'i'), ("òóọỏõôồốộổỗơờớợởỡ", 'o'), ("ùúụủũưừứựửữ", 'u'), ("ỳýỵỷỹ", 'y'), ("đ", 'd')];
    for (set, r) in MAP {
        if set.contains(c) {
            return *r;
        }
    }
    c
}

/// Parse và kiểm tra một SKILL.md. `dir_name` dùng làm tên mặc định (Claude Code mặc định tên = tên thư mục).
pub fn parse_skill_md(content: &str, dir_name: &str) -> (String, String, serde_json::Value, Vec<String>) {
    let mut warnings = vec![];
    let (fm, body) = split_frontmatter(content);
    let mut meta = serde_json::Value::Object(Default::default());
    if let Some(fm) = fm {
        if fm.len() > 64 * 1024 {
            warnings.push("frontmatter quá lớn, bỏ qua".into());
        } else {
            match serde_yaml::from_str::<serde_yaml::Value>(fm) {
                Ok(v @ serde_yaml::Value::Mapping(_)) => meta = serde_json::to_value(v).unwrap_or_default(),
                Ok(_) => warnings.push("frontmatter không phải mapping YAML".into()),
                Err(e) => warnings.push(format!("frontmatter YAML lỗi: {}", e.to_string().chars().take(160).collect::<String>())),
            }
        }
    } else {
        warnings.push("thiếu frontmatter (name/description) — một số công cụ AI có thể không nhận".into());
    }
    let str_field = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let name = str_field("name").unwrap_or_else(|| {
        if !dir_name.is_empty() {
            warnings.push("thiếu `name` — dùng tên thư mục".into());
        }
        dir_name.to_string()
    });
    let description = str_field("description").unwrap_or_else(|| {
        warnings.push("thiếu `description` — Codex và Antigravity yêu cầu trường này".into());
        body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("").chars().take(300).collect()
    });
    let name: String = name.chars().filter(|c| !c.is_control()).take(120).collect();
    let description: String = description.chars().filter(|c| !c.is_control() || *c == '\n').take(1024).collect();
    for k in ["hooks", "allowed-tools", "allowed_tools"] {
        if meta.get(k).is_some() {
            warnings.push(format!("khai báo `{k}` — cần xem kỹ quyền trước khi cài"));
        }
    }
    (name, description, meta, warnings)
}

fn is_script(p: &Path) -> bool {
    p.extension().map(|e| SCRIPT_EXTS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// Liệt kê file trong thư mục skill, không theo symlink.
pub fn list_files(skill_dir: &Path) -> (Vec<SkillFile>, Vec<String>) {
    let mut files = vec![];
    let mut warnings = vec![];
    for e in walkdir::WalkDir::new(skill_dir).follow_links(false).max_depth(8).into_iter().filter_map(Result::ok) {
        if e.depth() == 0 {
            continue;
        }
        let ft = e.file_type();
        let rel = e.path().strip_prefix(skill_dir).unwrap_or(e.path()).to_string_lossy().replace('\\', "/");
        if ft.is_symlink() {
            warnings.push(format!("bỏ qua symlink: {rel}"));
            continue;
        }
        if ft.is_file() {
            if files.len() >= MAX_FILES_PER_SKILL {
                warnings.push(format!("quá {MAX_FILES_PER_SKILL} file — cắt bớt danh sách"));
                break;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(SkillFile { executable: is_script(e.path()), path: rel, size });
        }
    }
    (files, warnings)
}

/// Quét `root` tìm mọi thư mục chứa SKILL.md (không phân biệt hoa thường), hỗ trợ lồng nhau.
pub fn scan(root: &Path) -> anyhow::Result<(Vec<ScannedSkill>, Vec<String>)> {
    anyhow::ensure!(root.is_dir(), "thư mục không tồn tại: {}", root.display());
    let root_c = doto_core::paths::dunce_canonical(root).unwrap_or_else(|| root.to_path_buf());
    let mut out = vec![];
    let mut errors = vec![];
    let walker = walkdir::WalkDir::new(&root_c).follow_links(false).max_depth(12).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        e.depth() == 0 || !(n == ".git" || n == "node_modules" || n == "target" || n == "__pycache__")
    });
    for e in walker.filter_map(Result::ok) {
        if !e.file_type().is_file() || !e.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md") {
            continue;
        }
        if out.len() >= MAX_SKILLS_PER_SOURCE {
            errors.push(format!("quá {MAX_SKILLS_PER_SOURCE} skill trong một nguồn — dừng quét"));
            break;
        }
        let path = e.path();
        let dir = path.parent().unwrap_or(&root_c).to_path_buf();
        let rel = dir.strip_prefix(&root_c).unwrap_or(Path::new("")).to_string_lossy().replace('\\', "/");
        match scan_one(&dir, path, &rel) {
            Ok(s) => out.push(s),
            Err(err) => errors.push(format!("{rel}: {err}")),
        }
    }
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok((out, errors))
}

fn scan_one(dir: &Path, skill_md: &Path, rel: &str) -> anyhow::Result<ScannedSkill> {
    let meta = std::fs::metadata(skill_md)?;
    anyhow::ensure!(meta.len() <= MAX_SKILL_MD_BYTES, "SKILL.md vượt {} KB", MAX_SKILL_MD_BYTES / 1024);
    let bytes = std::fs::read(skill_md)?;
    let content = String::from_utf8(bytes.clone()).map_err(|_| anyhow::anyhow!("SKILL.md không phải UTF-8"))?;
    let dir_name = dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let (name, description, metadata, mut warnings) = parse_skill_md(&content, &dir_name);
    let (files, fw) = list_files(dir);
    warnings.extend(fw);
    let has_scripts = files.iter().any(|f| f.executable);
    if has_scripts {
        warnings.push("chứa script/binary — Doto Skill không tự chạy; cần xem và xác nhận trước khi cài".into());
    }
    // revision = hash(SKILL.md + danh sách file & kích thước) — đổi khi nội dung skill đổi
    let mut h = String::from_utf8_lossy(&bytes).into_owned();
    for f in &files {
        h.push_str(&format!("\n{}:{}", f.path, f.size));
    }
    Ok(ScannedSkill {
        canonical_id: slugify(&name),
        name,
        description,
        rel_path: rel.to_string(),
        revision: doto_core::sha256_hex(h)[..16].to_string(),
        metadata,
        has_scripts,
        warnings,
    })
}

/// Thư mục skill tuyệt đối từ gốc nguồn + rel_path, đảm bảo vẫn nằm trong gốc.
pub fn resolve_skill_dir(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(!rel.split('/').any(|s| s == ".."), "đường dẫn không hợp lệ");
    let p = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    anyhow::ensure!(doto_core::paths::is_within(root, &p), "đường dẫn skill thoát khỏi thư mục nguồn");
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn frontmatter_split() {
        let (fm, body) = split_frontmatter("---\nname: a\ndescription: b\n---\n# Body\n");
        assert_eq!(fm, Some("name: a\ndescription: b\n"));
        assert_eq!(body, "# Body\n");
        assert_eq!(split_frontmatter("# no fm").0, None);
        assert_eq!(split_frontmatter("\u{feff}---\r\nname: x\r\n---\r\nbody").0, Some("name: x\r\n"));
    }

    #[test]
    fn slug_handles_vietnamese() {
        assert_eq!(slugify("Dựng Video Tin Tức"), "dung-video-tin-tuc");
        assert_eq!(slugify("  PDF tools!! "), "pdf-tools");
        assert_eq!(slugify("###"), "skill");
    }

    #[test]
    fn scans_nested_and_flags_scripts() {
        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("a/SKILL.md"), "---\nname: Alpha\ndescription: Does A\n---\nbody");
        write(&d.path().join("group/b/skill.md"), "# Beta\nNo frontmatter here");
        write(&d.path().join("group/b/scripts/run.sh"), "echo hi");
        write(&d.path().join("node_modules/x/SKILL.md"), "---\nname: ignored\n---");
        write(&d.path().join("c/SKILL.md"), "---\nname: [broken\n---\n");
        let (skills, errors) = scan(d.path()).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(skills.len(), 3);
        let a = skills.iter().find(|s| s.rel_path == "a").unwrap();
        assert_eq!(a.name, "Alpha");
        assert_eq!(a.canonical_id, "alpha");
        assert!(!a.has_scripts);
        let b = skills.iter().find(|s| s.rel_path == "group/b").unwrap();
        assert_eq!(b.name, "b");
        assert!(b.has_scripts);
        assert!(b.warnings.iter().any(|w| w.contains("script")));
        assert_eq!(b.description, "No frontmatter here");
        let c = skills.iter().find(|s| s.rel_path == "c").unwrap();
        assert!(c.warnings.iter().any(|w| w.contains("YAML")));
    }

    #[test]
    fn empty_folder_yields_zero_skills() {
        let d = tempfile::tempdir().unwrap();
        let (skills, errors) = scan(d.path()).unwrap();
        assert!(skills.is_empty() && errors.is_empty());
    }

    #[test]
    fn oversized_skill_md_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("big/SKILL.md"), &"x".repeat(MAX_SKILL_MD_BYTES as usize + 10));
        let (skills, errors) = scan(d.path()).unwrap();
        assert!(skills.is_empty());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn revision_changes_with_content() {
        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("a/SKILL.md"), "---\nname: A\ndescription: x\n---\n");
        let r1 = scan(d.path()).unwrap().0[0].revision.clone();
        write(&d.path().join("a/SKILL.md"), "---\nname: A\ndescription: y\n---\n");
        let r2 = scan(d.path()).unwrap().0[0].revision.clone();
        assert_ne!(r1, r2);
    }

    #[test]
    fn resolve_rejects_traversal() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("a")).unwrap();
        assert!(resolve_skill_dir(d.path(), "a").is_ok());
        assert!(resolve_skill_dir(d.path(), "../x").is_err());
    }
}

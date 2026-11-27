//! Import skill từ tệp nén (`.zip`, `.tar.gz`/`.tgz`, `.tar`, và `.7z`/`.rar` qua `tar` hệ thống) hoặc
//! từ thư mục tải lên bằng trình duyệt (`webkitdirectory`). Skill import là **input không tin cậy**:
//! - chặn Zip-Slip / đường dẫn tuyệt đối / `..`, bỏ symlink & hardlink, giới hạn số entry + dung lượng giải nén;
//! - không bao giờ thực thi script trong skill (chỉ sao chép tệp);
//! - sao chép vào thư mục skill tuỳ chỉnh rồi đăng ký thư mục đó như một nguồn `folder`.

pub mod archive;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

pub use archive::{
    archive_kind, archive_stem, extract_archive, extract_tar_plain, extract_zip, safe_relative, sanitize_extracted, ArchiveKind, MAX_ARCHIVE_BYTES,
    MAX_UPLOAD_FILES,
};

use crate::github::MAX_EXTRACTED_BYTES;
use crate::scanner;
use crate::service::{SkillError, SkillService};

/// Một "đơn vị skill" phát hiện được trong thư mục staging.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillUnit {
    pub dir: PathBuf,
    pub slug: String,
    pub needs_skill_md: bool,
}

fn has_skill_md(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md") && e.path().is_file()))
        .unwrap_or(false)
}

fn visible_children(dir: &Path) -> (Vec<PathBuf>, usize) {
    let mut dirs = vec![];
    let mut files = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(Result::ok) {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with('.') || n == "__MACOSX" {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => dirs.push(e.path()),
                Ok(t) if t.is_file() => files += 1,
                _ => {}
            }
        }
    }
    dirs.sort();
    (dirs, files)
}

fn skill_name_from(dir: &Path, fallback: &str) -> String {
    let md = std::fs::read_dir(dir).ok().and_then(|rd| rd.filter_map(Result::ok).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md")));
    if let Some(e) = md {
        if let Ok(c) = std::fs::read_to_string(e.path()) {
            let (name, _, _, _) = scanner::parse_skill_md(&c, fallback);
            if !name.trim().is_empty() {
                return name;
            }
        }
    }
    dir.file_name().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
}

/// Phát hiện skill: (1) gốc có SKILL.md; (2) các thư mục con cấp 1 có SKILL.md (bộ nhiều skill);
/// (3) một thư mục bọc duy nhất → xét bên trong; (4) `auto_md` → coi cả thư mục là một skill và sinh SKILL.md.
pub fn detect_skill_units(root: &Path, fallback_name: &str, auto_md: bool) -> Vec<SkillUnit> {
    let mut cur = root.to_path_buf();
    for _ in 0..3 {
        if has_skill_md(&cur) {
            let name = skill_name_from(&cur, fallback_name);
            return vec![SkillUnit { dir: cur, slug: scanner::slugify(&name), needs_skill_md: false }];
        }
        let (dirs, files) = visible_children(&cur);
        let with_md: Vec<PathBuf> = dirs.iter().filter(|d| has_skill_md(d)).cloned().collect();
        if !with_md.is_empty() {
            return with_md
                .into_iter()
                .map(|d| {
                    let fb = d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let name = skill_name_from(&d, &fb);
                    SkillUnit { dir: d, slug: scanner::slugify(&name), needs_skill_md: false }
                })
                .collect();
        }
        if dirs.len() == 1 && files == 0 {
            cur = dirs[0].clone();
            continue;
        }
        break;
    }
    if auto_md {
        let (dirs, files) = visible_children(&cur);
        if dirs.is_empty() && files == 0 {
            return vec![];
        }
        let name = if cur == root {
            fallback_name.to_string()
        } else {
            cur.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| fallback_name.into())
        };
        return vec![SkillUnit { dir: cur, slug: scanner::slugify(&name), needs_skill_md: true }];
    }
    vec![]
}

/// Sinh SKILL.md tối thiểu (frontmatter name + description) cho thư mục tài nguyên/prompt chưa có.
pub fn generated_skill_md(name: &str, files: &[String]) -> String {
    let safe = name.replace(['\n', '\r', '"'], " ");
    let mut s = format!(
        "---\nname: {}\ndescription: \"Skill được Doto Skill tạo tự động từ thư mục tải lên — hãy chỉnh lại mô tả.\"\n---\n\n# {}\n\nTài nguyên đi kèm:\n",
        scanner::slugify(&safe),
        safe.trim()
    );
    for f in files.iter().take(50) {
        s.push_str(&format!("- `{f}`\n"));
    }
    s
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<usize> {
    let mut n = 0;
    for e in walkdir::WalkDir::new(src).follow_links(false).max_depth(16).into_iter().filter_map(Result::ok) {
        let rel = e.path().strip_prefix(src).unwrap_or(Path::new(""));
        let name = e.file_name().to_string_lossy();
        if rel.as_os_str().is_empty() {
            continue;
        }
        if name == "__MACOSX" || name == ".DS_Store" || rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        let out = dst.join(rel);
        let ft = e.file_type();
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else if ft.is_file() {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(e.path(), &out)?;
            n += 1;
        }
    }
    Ok(n)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSkill {
    pub slug: String,
    pub name: String,
    pub files: usize,
    pub has_scripts: bool,
    pub generated_skill_md: bool,
    pub replaced: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<ImportedSkill>,
    pub skipped: Vec<String>,
    pub source_id: Option<String>,
    pub target_dir: String,
}

/// Sao chép các đơn vị skill từ staging vào `target_root` (blocking).
pub fn install_units(units: &[SkillUnit], target_root: &Path, overwrite: bool) -> Result<(Vec<ImportedSkill>, Vec<String>), SkillError> {
    std::fs::create_dir_all(target_root).map_err(anyhow::Error::from)?;
    let mut imported = vec![];
    let mut skipped = vec![];
    for u in units {
        let dest = target_root.join(&u.slug);
        let mut replaced = false;
        if dest.exists() {
            if !overwrite {
                skipped.push(format!("skill `{}` đã tồn tại — bật \"Ghi đè\" để thay thế", u.slug));
                continue;
            }
            if !doto_core::paths::is_within(target_root, &dest) {
                return Err(SkillError::Validation("đường dẫn đích không hợp lệ".into()));
            }
            std::fs::remove_dir_all(&dest).map_err(anyhow::Error::from)?;
            replaced = true;
        }
        let files = copy_tree(&u.dir, &dest).map_err(anyhow::Error::from)?;
        if u.needs_skill_md {
            let (list, _) = scanner::list_files(&dest);
            let names: Vec<String> = list.into_iter().map(|f| f.path).collect();
            std::fs::write(dest.join("SKILL.md"), generated_skill_md(&u.slug, &names)).map_err(anyhow::Error::from)?;
        }
        let (list, _) = scanner::list_files(&dest);
        let name = skill_name_from(&dest, &u.slug);
        imported.push(ImportedSkill {
            slug: u.slug.clone(),
            name,
            files,
            has_scripts: list.iter().any(|f| f.executable),
            generated_skill_md: u.needs_skill_md,
            replaced,
        });
    }
    Ok((imported, skipped))
}

/// Ghi các tệp tải lên từ trình duyệt (đường dẫn tương đối) vào staging, chặn traversal.
pub fn write_uploaded_files(staging: &Path, files: &[(String, Vec<u8>)]) -> Result<usize, SkillError> {
    if files.is_empty() {
        return Err(SkillError::Validation("thư mục tải lên trống".into()));
    }
    if files.len() > MAX_UPLOAD_FILES {
        return Err(SkillError::Validation(format!("tối đa {MAX_UPLOAD_FILES} tệp mỗi lần tải")));
    }
    let mut total: u64 = 0;
    for (raw, data) in files {
        let rel = safe_relative(raw).ok_or_else(|| SkillError::Validation(format!("đường dẫn không an toàn: {raw}")))?;
        total += data.len() as u64;
        if total > MAX_EXTRACTED_BYTES {
            return Err(SkillError::Validation("tổng dung lượng thư mục vượt giới hạn an toàn".into()));
        }
        let out = staging.join(rel);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(anyhow::Error::from)?;
        }
        std::fs::write(out, data).map_err(anyhow::Error::from)?;
    }
    Ok(files.len())
}

impl SkillService {
    /// Đưa thư mục staging (đã giải nén / đã ghi) vào thư mục skill tuỳ chỉnh và đăng ký làm nguồn `folder`.
    pub async fn import_staged(
        self: &Arc<Self>,
        staging: PathBuf,
        target_root: PathBuf,
        fallback_name: String,
        overwrite: bool,
        auto_md: bool,
        mut skipped: Vec<String>,
    ) -> Result<ImportReport, SkillError> {
        let tr = target_root.clone();
        let (imported, more) = tokio::task::spawn_blocking(move || {
            let units = detect_skill_units(&staging, &fallback_name, auto_md);
            if units.is_empty() {
                return Err(SkillError::Validation(
                    "không tìm thấy SKILL.md trong nội dung tải lên — bật \"Tự tạo SKILL.md\" nếu đây là thư mục prompt/tài nguyên".into(),
                ));
            }
            install_units(&units, &tr, overwrite)
        })
        .await
        .map_err(anyhow::Error::from)??;
        skipped.extend(more);
        let source_id = if imported.is_empty() { None } else { Some(self.ensure_folder_source(&target_root).await?) };
        Ok(ImportReport { imported, skipped, source_id, target_dir: target_root.to_string_lossy().into_owned() })
    }

    /// Tìm nguồn `folder` trỏ tới thư mục; chưa có thì thêm mới; có rồi thì quét lại.
    pub async fn ensure_folder_source(self: &Arc<Self>, dir: &Path) -> Result<String, SkillError> {
        let canon = doto_core::paths::dunce_canonical(dir).ok_or_else(|| SkillError::Validation("thư mục skill tuỳ chỉnh không tồn tại".into()))?;
        let loc = canon.to_string_lossy().into_owned();
        let existing = self.list_sources().await?.into_iter().find(|s| s.kind == "folder" && s.location == loc);
        match existing {
            Some(s) => {
                self.rescan(&s.id).await?;
                Ok(s.id)
            }
            None => Ok(self.add_folder(&loc, true).await?.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tạo ZIP tối giản (Store/Deflate) cho test — không cần crate ngoài.
    pub(crate) fn build_zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        use std::io::Write;
        let mut out = vec![];
        let mut central = vec![];
        for (name, data, deflate) in entries {
            let mut crc = flate2::Crc::new();
            crc.update(data);
            let comp = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            let off = out.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0]);
            out.extend_from_slice(&crc.sum().to_le_bytes());
            out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&comp);
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0]);
            central.extend_from_slice(&crc.sum().to_le_bytes());
            central.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn zip_extracts_store_and_deflate_and_blocks_zip_slip() {
        let d = tempfile::tempdir().unwrap();
        let z = build_zip(&[
            ("pack/a/SKILL.md", b"---\nname: Alpha\ndescription: A\n---\n", true),
            ("pack/a/ref.txt", b"hello", false),
            ("../evil.txt", b"x", false),
            ("C:/abs.txt", b"x", false),
        ]);
        let r = extract_zip(&z, d.path()).unwrap();
        assert_eq!(r.files, 2);
        assert_eq!(std::fs::read_to_string(d.path().join("pack/a/ref.txt")).unwrap(), "hello");
        assert!(!d.path().parent().unwrap().join("evil.txt").exists());
        assert!(r.skipped.iter().any(|s| s.contains("evil")));
    }

    #[test]
    fn zip_rejects_garbage_and_bad_crc() {
        let d = tempfile::tempdir().unwrap();
        assert!(extract_zip(b"not a zip at all", d.path()).is_err());
        let mut z = build_zip(&[("a.txt", b"hello world", false)]);
        z[36] ^= 0xFF;
        assert!(extract_zip(&z, d.path()).unwrap_err().to_string().contains("CRC"));
    }

    #[test]
    fn safe_relative_rules() {
        assert_eq!(safe_relative("a/b/c.md"), Some(PathBuf::from("a").join("b").join("c.md")));
        assert_eq!(safe_relative("./a\\b"), Some(PathBuf::from("a").join("b")));
        for bad in ["../x", "a/../../x", "/etc/passwd", "C:/x", "", "a/\0b"] {
            assert!(safe_relative(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn detects_single_wrapper_multi_and_auto_md() {
        let d = tempfile::tempdir().unwrap();
        let w = d.path().join("wrap");
        std::fs::create_dir_all(w.join("one")).unwrap();
        std::fs::write(w.join("one/SKILL.md"), "---\nname: One Skill\ndescription: x\n---").unwrap();
        std::fs::create_dir_all(w.join("two")).unwrap();
        std::fs::write(w.join("two/skill.md"), "---\nname: Two\ndescription: y\n---").unwrap();
        let u = detect_skill_units(d.path(), "fb", false);
        assert_eq!(u.iter().map(|x| x.slug.as_str()).collect::<Vec<_>>(), vec!["one-skill", "two"]);

        let p = tempfile::tempdir().unwrap();
        std::fs::write(p.path().join("prompt.txt"), "hi").unwrap();
        assert!(detect_skill_units(p.path(), "Bộ prompt", false).is_empty());
        let u = detect_skill_units(p.path(), "Bộ prompt", true);
        assert_eq!(u.len(), 1);
        assert!(u[0].needs_skill_md);
        assert_eq!(u[0].slug, "bo-prompt");

        let target = tempfile::tempdir().unwrap();
        let (imp, _) = install_units(&u, target.path(), false).unwrap();
        assert!(imp[0].generated_skill_md);
        assert!(target.path().join("bo-prompt/SKILL.md").is_file());
        let (imp2, sk) = install_units(&u, target.path(), false).unwrap();
        assert!(imp2.is_empty() && sk.len() == 1);
        let (imp3, _) = install_units(&u, target.path(), true).unwrap();
        assert!(imp3[0].replaced);
    }

    #[test]
    fn tar_gz_plain_keeps_root_and_blocks_traversal() {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (p, data) in [("SKILL.md", &b"---\nname: T\ndescription: d\n---"[..]), ("../../escape.txt", b"x")] {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            let name = p.as_bytes();
            h.as_old_mut().name[..name.len()].copy_from_slice(name);
            h.set_cksum();
            b.append(&h, data).unwrap();
        }
        let gz = b.into_inner().unwrap().finish().unwrap();
        let d = tempfile::tempdir().unwrap();
        let r = extract_tar_plain(&gz[..], true, d.path()).unwrap();
        assert_eq!(r.files, 1);
        assert!(d.path().join("SKILL.md").is_file());
        assert!(r.skipped.iter().any(|s| s.contains("escape")));
    }

    #[test]
    fn archive_kind_and_stem() {
        assert_eq!(archive_kind("A.ZIP"), Some(ArchiveKind::Zip));
        assert_eq!(archive_kind("x.tgz"), Some(ArchiveKind::TarGz));
        assert_eq!(archive_kind("x.7z"), Some(ArchiveKind::SystemTar));
        assert_eq!(archive_kind("x.exe"), None);
        assert_eq!(archive_stem("C:/tmp/My Skill.tar.gz"), "My Skill");
    }

    #[test]
    fn uploaded_files_reject_traversal() {
        let d = tempfile::tempdir().unwrap();
        assert!(write_uploaded_files(d.path(), &[("../x".into(), b"1".to_vec())]).is_err());
        assert_eq!(write_uploaded_files(d.path(), &[("dir/SKILL.md".into(), b"1".to_vec())]).unwrap(), 1);
    }
}

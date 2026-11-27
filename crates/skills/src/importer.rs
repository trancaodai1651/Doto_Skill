//! Import skill từ tệp nén (`.zip`, `.tar.gz`/`.tgz`, `.tar`, và `.7z`/`.rar` qua `tar` hệ thống) hoặc
//! từ thư mục tải lên bằng trình duyệt (`webkitdirectory`). Skill import là **input không tin cậy**:
//! - chặn Zip-Slip / đường dẫn tuyệt đối / `..`, bỏ symlink & hardlink, giới hạn số entry + dung lượng giải nén;
//! - không bao giờ thực thi script trong skill (chỉ sao chép tệp);
//! - sao chép vào thư mục skill tuỳ chỉnh rồi đăng ký thư mục đó như một nguồn `folder`.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::github::{ExtractReport, MAX_ENTRIES, MAX_EXTRACTED_BYTES};
use crate::scanner;
use crate::service::{SkillError, SkillService};

/// Dung lượng tối đa của tệp nén tải lên.
pub const MAX_ARCHIVE_BYTES: u64 = 200 * 1024 * 1024;
/// Số tệp tối đa khi tải thư mục từ trình duyệt.
pub const MAX_UPLOAD_FILES: usize = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    TarGz,
    Tar,
    /// `.7z`, `.rar`, `.tar.xz`, `.tar.bz2`: giải nén bằng `tar` (bsdtar/libarchive) có sẵn trên Windows 10+/macOS.
    SystemTar,
}

pub fn archive_kind(filename: &str) -> Option<ArchiveKind> {
    let f = filename.to_ascii_lowercase();
    if f.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if f.ends_with(".tar.gz") || f.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if f.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else if f.ends_with(".7z") || f.ends_with(".rar") || f.ends_with(".tar.xz") || f.ends_with(".txz") || f.ends_with(".tar.bz2") || f.ends_with(".tbz2") {
        Some(ArchiveKind::SystemTar)
    } else {
        None
    }
}

/// Tên gốc (bỏ đuôi nén) dùng làm tên skill mặc định.
pub fn archive_stem(filename: &str) -> String {
    let base = Path::new(filename).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let lower = base.to_ascii_lowercase();
    for ext in [".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".txz", ".tbz2", ".zip", ".tar", ".7z", ".rar"] {
        if lower.ends_with(ext) {
            return base[..base.len() - ext.len()].to_string();
        }
    }
    base
}

/// Chuẩn hoá đường dẫn tương đối trong archive/thư mục upload: chỉ chấp nhận component `Normal`.
pub fn safe_relative(raw: &str) -> Option<PathBuf> {
    let norm = raw.replace('\\', "/");
    if norm.starts_with('/') || norm.contains('\0') || (norm.len() >= 2 && norm.as_bytes()[1] == b':') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in norm.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() || out.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(out)
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn bad(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into())
}

/// Giải nén ZIP thuần Rust (Store + Deflate) qua central directory, kiểm tra CRC-32, chống Zip-Slip & zip bomb.
pub fn extract_zip(data: &[u8], dest: &Path) -> std::io::Result<ExtractReport> {
    std::fs::create_dir_all(dest)?;
    let mut report = ExtractReport::default();
    // Tìm End Of Central Directory (EOCD) — tối đa 64 KiB comment.
    let min = data.len().saturating_sub(22 + 65_535);
    let eocd = (min..data.len().saturating_sub(21)).rev().find(|&i| u32le(data, i) == Some(0x0605_4b50)).ok_or_else(|| bad("không phải tệp ZIP hợp lệ"))?;
    let total = u16le(data, eocd + 10).ok_or_else(|| bad("EOCD hỏng"))? as usize;
    let cd_off = u32le(data, eocd + 16).ok_or_else(|| bad("EOCD hỏng"))? as usize;
    if total == 0xFFFF || cd_off == 0xFFFF_FFFF {
        return Err(bad("ZIP64 chưa được hỗ trợ — hãy nén lại bằng .tar.gz"));
    }
    if total > MAX_ENTRIES {
        return Err(bad("archive có quá nhiều file"));
    }
    let mut p = cd_off;
    for _ in 0..total {
        if u32le(data, p) != Some(0x0201_4b50) {
            return Err(bad("central directory hỏng"));
        }
        let flags = u16le(data, p + 8).unwrap_or(0);
        let method = u16le(data, p + 10).unwrap_or(0);
        let crc = u32le(data, p + 16).unwrap_or(0);
        let csize = u32le(data, p + 20).unwrap_or(0) as usize;
        let usize_ = u32le(data, p + 24).unwrap_or(0) as u64;
        let nlen = u16le(data, p + 28).unwrap_or(0) as usize;
        let xlen = u16le(data, p + 30).unwrap_or(0) as usize;
        let clen = u16le(data, p + 32).unwrap_or(0) as usize;
        let ext_attr = u32le(data, p + 38).unwrap_or(0);
        let loff = u32le(data, p + 42).unwrap_or(0) as usize;
        let name_bytes = data.get(p + 46..p + 46 + nlen).ok_or_else(|| bad("tên entry hỏng"))?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        p += 46 + nlen + xlen + clen;

        let is_dir = name.ends_with('/') || name.ends_with('\\');
        let unix_mode = ext_attr >> 16;
        if unix_mode & 0o170000 == 0o120000 {
            report.skipped.push(format!("bỏ qua symlink: {name}"));
            continue;
        }
        let Some(rel) = safe_relative(&name) else {
            if !is_dir || name.contains("..") {
                report.skipped.push(format!("đường dẫn không an toàn: {name}"));
            }
            continue;
        };
        let out = dest.join(&rel);
        if is_dir {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if flags & 1 != 0 {
            report.skipped.push(format!("bỏ qua tệp mã hoá: {name}"));
            continue;
        }
        report.bytes += usize_;
        if report.bytes > MAX_EXTRACTED_BYTES {
            return Err(bad("dung lượng giải nén vượt giới hạn an toàn (zip bomb?)"));
        }
        if u32le(data, loff) != Some(0x0403_4b50) {
            return Err(bad(format!("local header hỏng: {name}")));
        }
        let lnlen = u16le(data, loff + 26).unwrap_or(0) as usize;
        let lxlen = u16le(data, loff + 28).unwrap_or(0) as usize;
        let start = loff + 30 + lnlen + lxlen;
        let comp = data.get(start..start + csize).ok_or_else(|| bad(format!("dữ liệu entry bị cắt: {name}")))?;
        let mut buf = Vec::with_capacity(usize_.min(64 * 1024 * 1024) as usize);
        match method {
            0 => buf.extend_from_slice(comp),
            8 => {
                flate2::read::DeflateDecoder::new(comp).take(usize_ + 1).read_to_end(&mut buf)?;
            }
            m => {
                report.skipped.push(format!("phương thức nén {m} không hỗ trợ: {name}"));
                continue;
            }
        }
        if buf.len() as u64 != usize_ {
            return Err(bad(format!("kích thước entry không khớp: {name}")));
        }
        let mut h = flate2::Crc::new();
        h.update(&buf);
        if h.sum() != crc {
            return Err(bad(format!("CRC-32 sai: {name}")));
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out, &buf)?;
        report.files += 1;
    }
    Ok(report)
}

/// Giải nén tar (có/không gzip) **không** bỏ thư mục gốc, cùng các lớp bảo vệ như GitHub import.
pub fn extract_tar_plain<R: Read>(reader: R, gz: bool, dest: &Path) -> std::io::Result<ExtractReport> {
    std::fs::create_dir_all(dest)?;
    let inner: Box<dyn Read> = if gz { Box::new(flate2::read::GzDecoder::new(reader)) } else { Box::new(reader) };
    let mut archive = tar::Archive::new(inner);
    let mut report = ExtractReport::default();
    for (n, entry) in archive.entries()?.enumerate() {
        if n >= MAX_ENTRIES {
            return Err(bad("archive có quá nhiều file"));
        }
        let mut entry = entry?;
        let raw = entry.path()?.to_string_lossy().into_owned();
        let et = entry.header().entry_type();
        if et.is_symlink() || et.is_hard_link() {
            report.skipped.push(format!("bỏ qua link: {raw}"));
            continue;
        }
        let Some(rel) = safe_relative(&raw) else {
            if raw.contains("..") || raw.starts_with('/') {
                report.skipped.push(format!("đường dẫn không an toàn: {raw}"));
            }
            continue;
        };
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
            return Err(bad("dung lượng giải nén vượt giới hạn an toàn (tar bomb?)"));
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        let copied = std::io::copy(&mut (&mut entry).take(size + 1), &mut f)?;
        if copied > size {
            return Err(bad("kích thước entry không khớp header"));
        }
        report.files += 1;
    }
    Ok(report)
}

fn system_tar() -> Option<PathBuf> {
    let cands: Vec<PathBuf> = if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        vec![root.join("System32").join("tar.exe")]
    } else {
        vec![PathBuf::from("/usr/bin/bsdtar"), PathBuf::from("/usr/bin/tar"), PathBuf::from("/bin/tar")]
    };
    cands.into_iter().find(|p| p.is_file())
}

/// Sau khi `tar` hệ thống giải nén: xoá mọi symlink/junction, kiểm tra mọi tệp vẫn nằm trong thư mục đích, đếm giới hạn.
pub fn sanitize_extracted(dest: &Path) -> std::io::Result<ExtractReport> {
    let mut report = ExtractReport::default();
    let root = doto_core::paths::dunce_canonical(dest).ok_or_else(|| bad("không đọc được thư mục giải nén"))?;
    let mut links = vec![];
    for e in walkdir::WalkDir::new(dest).follow_links(false).into_iter().filter_map(Result::ok) {
        if e.depth() == 0 {
            continue;
        }
        let ft = e.file_type();
        if ft.is_symlink() || is_reparse_point(e.path()) {
            links.push(e.path().to_path_buf());
            continue;
        }
        if ft.is_file() {
            let c = doto_core::paths::dunce_canonical(e.path()).unwrap_or_default();
            if !c.starts_with(&root) {
                return Err(bad("tệp giải nén thoát khỏi thư mục đích"));
            }
            report.files += 1;
            report.bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            if report.files > MAX_ENTRIES || report.bytes > MAX_EXTRACTED_BYTES {
                return Err(bad("dung lượng/số tệp giải nén vượt giới hạn an toàn"));
            }
        }
    }
    for l in links {
        report.skipped.push(format!("đã xoá link: {}", l.strip_prefix(dest).unwrap_or(&l).display()));
        let _ = std::fs::remove_file(&l).or_else(|_| std::fs::remove_dir(&l));
    }
    Ok(report)
}

#[cfg(windows)]
fn is_reparse_point(p: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(p).map(|m| m.file_attributes() & 0x400 != 0).unwrap_or(false)
}
#[cfg(not(windows))]
fn is_reparse_point(_p: &Path) -> bool {
    false
}

/// Giải nén tệp nén vào `dest` theo loại (blocking — gọi trong `spawn_blocking`).
pub fn extract_archive(file: &Path, filename: &str, dest: &Path) -> std::io::Result<ExtractReport> {
    let kind = archive_kind(filename).ok_or_else(|| bad("định dạng không hỗ trợ — dùng .zip, .tar.gz, .tgz, .tar, .7z hoặc .rar"))?;
    let len = std::fs::metadata(file)?.len();
    if len > MAX_ARCHIVE_BYTES {
        return Err(bad("tệp nén vượt 200 MB"));
    }
    match kind {
        ArchiveKind::Zip => extract_zip(&std::fs::read(file)?, dest),
        ArchiveKind::TarGz => extract_tar_plain(std::fs::File::open(file)?, true, dest),
        ArchiveKind::Tar => extract_tar_plain(std::fs::File::open(file)?, false, dest),
        ArchiveKind::SystemTar => {
            let tar = system_tar().ok_or_else(|| bad("máy chủ không có `tar` hệ thống để giải nén .7z/.rar — hãy dùng .zip hoặc .tar.gz"))?;
            std::fs::create_dir_all(dest)?;
            let mut cmd = std::process::Command::new(tar);
            cmd.arg("-xf").arg(file).arg("-C").arg(dest).stdin(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            let out = cmd.output()?;
            if !out.status.success() {
                let msg = doto_core::redact_secrets(&String::from_utf8_lossy(&out.stderr));
                return Err(bad(format!("tar không giải nén được: {}", msg.chars().take(300).collect::<String>())));
            }
            sanitize_extracted(dest)
        }
    }
}

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
        // hỏng 1 byte dữ liệu (sau header 30 + tên 5)
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
        // lần 2 không ghi đè → bỏ qua; ghi đè → replaced
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

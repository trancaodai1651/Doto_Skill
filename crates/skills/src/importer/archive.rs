//! Giải nén an toàn gói skill (`.zip` thuần Rust có kiểm CRC-32, `.tar.gz`/`.tar`, và `.7z`/`.rar` qua `tar` hệ thống)
//! kèm bộ lọc đường dẫn tương đối (`safe_relative`) và hậu kiểm dọn symlink/reparse-point (`sanitize_extracted`).

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::github::{ExtractReport, MAX_ENTRIES, MAX_EXTRACTED_BYTES};

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
            let mut cmd = doto_core::process::headless_std_command(tar);
            cmd.arg("-xf").arg(file).arg("-C").arg(dest);
            let out = cmd.output()?;
            if !out.status.success() {
                let msg = doto_core::redact_secrets(&String::from_utf8_lossy(&out.stderr));
                return Err(bad(format!("tar không giải nén được: {}", msg.chars().take(300).collect::<String>())));
            }
            sanitize_extracted(dest)
        }
    }
}

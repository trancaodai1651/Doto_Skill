use std::path::{Path, PathBuf};

/// Các thư mục dữ liệu của ứng dụng. Mặc định theo chuẩn OS:
/// - Windows: `%APPDATA%\Doto\DotoSkill\data`
/// - macOS: `~/Library/Application Support/com.doto.DotoSkill`
///
/// Có thể ghi đè bằng biến môi trường `DOTO_DATA_DIR` hoặc tham số dòng lệnh.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(override_dir: Option<&Path>) -> anyhow::Result<Self> {
        let data_dir = if let Some(p) = override_dir {
            p.to_path_buf()
        } else if let Ok(p) = std::env::var("DOTO_DATA_DIR") {
            PathBuf::from(p)
        } else {
            directories::ProjectDirs::from("com", "doto", "DotoSkill")
                .ok_or_else(|| anyhow::anyhow!("không xác định được thư mục dữ liệu người dùng"))?
                .data_dir()
                .to_path_buf()
        };
        let paths = Self { data_dir };
        paths.ensure()?;
        Ok(paths)
    }

    fn ensure(&self) -> anyhow::Result<()> {
        for dir in [
            self.data_dir.clone(),
            self.thumbnails_dir(),
            self.default_exports_dir(),
            self.default_uploads_dir(),
            self.default_custom_skills_dir(),
            self.skills_cache_dir(),
            self.backups_dir(),
            self.logs_dir(),
            self.tmp_dir(),
        ] {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("doto.sqlite")
    }
    pub fn thumbnails_dir(&self) -> PathBuf {
        self.data_dir.join("thumbnails")
    }
    pub fn default_exports_dir(&self) -> PathBuf {
        self.data_dir.join("exports")
    }
    pub fn default_uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }
    pub fn default_custom_skills_dir(&self) -> PathBuf {
        self.data_dir.join("custom-skills")
    }
    pub fn default_workspace_memory_dir(&self) -> PathBuf {
        self.data_dir.join("workspace-memory")
    }
    pub fn skills_cache_dir(&self) -> PathBuf {
        self.data_dir.join("skills-cache")
    }
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
    pub fn tmp_dir(&self) -> PathBuf {
        self.data_dir.join("tmp")
    }
}

/// Kiểm tra `candidate` nằm trong `root` sau khi chuẩn hoá (chống path traversal / symlink escape).
pub fn is_within(root: &Path, candidate: &Path) -> bool {
    match (dunce_canonical(root), dunce_canonical(candidate)) {
        (Some(r), Some(c)) => c.starts_with(r),
        _ => false,
    }
}

/// canonicalize nhưng bỏ tiền tố `\\?\` của Windows để so sánh/hiển thị nhất quán.
pub fn dunce_canonical(p: &Path) -> Option<PathBuf> {
    let c = std::fs::canonicalize(p).ok()?;
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            if !rest.starts_with("UNC\\") {
                return Some(PathBuf::from(rest));
            }
        }
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_detects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("a");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("f.txt"), b"x").unwrap();
        assert!(is_within(dir.path(), &inner.join("f.txt")));
        assert!(!is_within(&inner, &inner.join("..").join("..")));
    }
}

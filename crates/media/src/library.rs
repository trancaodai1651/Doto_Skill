//! Thư viện video (bảng `video_assets`): quét thư mục nguồn được cấp quyền, lập chỉ mục metadata.

use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use doto_core::Db;

pub const VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm", "avi"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoAsset {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub has_thumbnail: bool,
    pub visibility: String,
    pub status: String,
    pub created_at: String,
    pub job_id: Option<String>,
    /// Đường dẫn thật trên đĩa — KHÔNG bao giờ serialize ra API.
    #[serde(skip)]
    pub file_path: PathBuf,
    #[serde(skip)]
    pub thumbnail_path: Option<PathBuf>,
    #[serde(skip)]
    pub has_audio: bool,
}

const COLS: &str =
    "id,kind,title,file_name,size_bytes,duration_ms,width,height,video_codec,audio_codec,thumbnail_path,visibility,status,created_at,job_id,file_path";

fn from_row(r: &Row) -> rusqlite::Result<VideoAsset> {
    let thumb: Option<String> = r.get(10)?;
    let audio: Option<String> = r.get(9)?;
    Ok(VideoAsset {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        file_name: r.get(3)?,
        size_bytes: r.get(4)?,
        duration_ms: r.get(5)?,
        width: r.get(6)?,
        height: r.get(7)?,
        video_codec: r.get(8)?,
        has_audio: audio.is_some(),
        audio_codec: audio,
        has_thumbnail: thumb.as_deref().map(|p| Path::new(p).is_file()).unwrap_or(false),
        thumbnail_path: thumb.map(PathBuf::from),
        visibility: r.get(11)?,
        status: r.get(12)?,
        created_at: r.get(13)?,
        job_id: r.get(14)?,
        file_path: PathBuf::from(r.get::<_, String>(15)?),
    })
}

#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub kind: Option<String>,
    pub q: Option<String>,
    /// Viewer: chỉ export có visibility='viewers' và status='ready'.
    pub viewer_only: bool,
}

pub async fn list(db: &Db, f: ListFilter) -> anyhow::Result<Vec<VideoAsset>> {
    db.call(move |c| {
        let mut sql = format!("SELECT {COLS} FROM video_assets WHERE 1=1");
        let mut args: Vec<String> = vec![];
        if f.viewer_only {
            sql.push_str(" AND kind='export' AND visibility='viewers' AND status='ready'");
        }
        if let Some(k) = f.kind.filter(|k| k == "source" || k == "export") {
            args.push(k);
            sql.push_str(&format!(" AND kind=?{}", args.len()));
        }
        if let Some(q) = f.q.filter(|q| !q.trim().is_empty()) {
            args.push(format!("%{}%", q.trim().replace('%', "")));
            sql.push_str(&format!(" AND (title LIKE ?{0} OR file_name LIKE ?{0})", args.len()));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 1000");
        let mut st = c.prepare(&sql)?;
        let v = st.query_map(rusqlite::params_from_iter(args.iter()), from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .await
}

pub async fn get(db: &Db, id: &str) -> anyhow::Result<Option<VideoAsset>> {
    let id = id.to_string();
    db.call(move |c| Ok(c.query_row(&format!("SELECT {COLS} FROM video_assets WHERE id=?1"), [id], from_row).optional()?)).await
}

pub struct NewAsset<'a> {
    pub kind: &'a str,
    pub title: String,
    pub path: &'a Path,
    pub size: i64,
    pub mtime: Option<String>,
    pub info: &'a crate::probe::MediaInfo,
    pub visibility: &'a str,
    pub job_id: Option<String>,
}

/// Thêm hoặc cập nhật theo `file_path` (UNIQUE). Trả id.
pub async fn upsert(db: &Db, a: NewAsset<'_>) -> anyhow::Result<String> {
    let id = doto_core::new_id();
    let file_name = a.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let p = a.path.to_string_lossy().into_owned();
    let (kind, title, vis, mtime, job) = (a.kind.to_string(), a.title, a.visibility.to_string(), a.mtime, a.job_id);
    let i = a.info.clone();
    let size = a.size;
    db.call(move |c| {
        c.execute(
            "INSERT INTO video_assets(id,kind,title,file_path,file_name,size_bytes,mtime,duration_ms,width,height,video_codec,audio_codec,container,visibility,status,job_id,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'ready',?15,?16)
             ON CONFLICT(file_path) DO UPDATE SET size_bytes=excluded.size_bytes, mtime=excluded.mtime, duration_ms=excluded.duration_ms,
               width=excluded.width, height=excluded.height, video_codec=excluded.video_codec, audio_codec=excluded.audio_codec,
               container=excluded.container, status='ready'",
            params![id, kind, title, p, file_name, size, mtime, i.duration_ms, i.width, i.height, i.video_codec, i.audio_codec, i.container, vis, job, doto_core::now_rfc3339()],
        )?;
        let real: String = c.query_row("SELECT id FROM video_assets WHERE file_path=?1", [p], |r| r.get(0))?;
        Ok(real)
    })
    .await
}

pub async fn set_thumbnail(db: &Db, id: &str, thumb: &Path) -> anyhow::Result<()> {
    let (id, t) = (id.to_string(), thumb.to_string_lossy().into_owned());
    db.call(move |c| {
        c.execute("UPDATE video_assets SET thumbnail_path=?2 WHERE id=?1", params![id, t])?;
        Ok(())
    })
    .await
}

pub async fn update_meta(db: &Db, id: &str, title: Option<String>, visibility: Option<String>) -> anyhow::Result<bool> {
    let id = id.to_string();
    if let Some(v) = &visibility {
        anyhow::ensure!(v == "private" || v == "viewers", "visibility không hợp lệ");
    }
    if let Some(t) = &title {
        anyhow::ensure!(!t.trim().is_empty() && t.chars().count() <= 200, "tiêu đề phải từ 1–200 ký tự");
    }
    db.call(move |c| {
        let n = c.execute(
            "UPDATE video_assets SET title=COALESCE(?2,title), visibility=COALESCE(?3,visibility) WHERE id=?1",
            params![id, title.map(|t| t.trim().to_string()), visibility],
        )?;
        Ok(n == 1)
    })
    .await
}

pub async fn delete(db: &Db, id: &str) -> anyhow::Result<Option<VideoAsset>> {
    let a = get(db, id).await?;
    if a.is_some() {
        let id = id.to_string();
        db.call(move |c| {
            c.execute("DELETE FROM video_assets WHERE id=?1", [id])?;
            Ok(())
        })
        .await?;
    }
    Ok(a)
}

pub async fn mark_missing(db: &Db, existing: Vec<String>) -> anyhow::Result<usize> {
    db.call(move |c| {
        let mut n = 0;
        let mut st = c.prepare("SELECT id, file_path FROM video_assets WHERE status<>'missing'")?;
        let rows: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        for (id, p) in rows {
            if !Path::new(&p).is_file() && !existing.contains(&p) {
                c.execute("UPDATE video_assets SET status='missing' WHERE id=?1", [id])?;
                n += 1;
            }
        }
        Ok(n)
    })
    .await
}

pub async fn known_files(db: &Db) -> anyhow::Result<std::collections::HashMap<String, (i64, Option<String>)>> {
    db.call(|c| {
        let mut st = c.prepare("SELECT file_path, size_bytes, mtime FROM video_assets WHERE status='ready'")?;
        let m = st.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?))))?.collect::<Result<_, _>>()?;
        Ok(m)
    })
    .await
}

/// Liệt kê file video trong thư mục được cấp quyền: không theo symlink, bỏ file ẩn / `.part`.
pub fn discover_files(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|x| VIDEO_EXTS.contains(&x.to_string_lossy().to_ascii_lowercase().as_str())).unwrap_or(false))
        .map(|e| e.into_path())
        .take(20_000)
        .collect()
}

/// Tên file an toàn từ tiêu đề do người dùng nhập.
pub fn sanitize_file_stem(title: &str) -> String {
    let s: String = title.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.chars().take(80).collect();
    let s = s.trim_matches(|c| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "video".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_titles() {
        assert_eq!(sanitize_file_stem("Bản tin ../../etc/passwd"), "Bản tin ______etc_passwd");
        assert_eq!(sanitize_file_stem("   "), "video");
        assert_eq!(sanitize_file_stem("a:b*c?"), "a_b_c_");
        assert!(sanitize_file_stem(&"x".repeat(300)).len() <= 80);
    }

    #[test]
    fn discovers_only_videos_and_skips_hidden() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.MP4"), b"x").unwrap();
        std::fs::write(d.path().join("b.txt"), b"x").unwrap();
        std::fs::write(d.path().join(".c.part.mp4"), b"x").unwrap();
        std::fs::create_dir(d.path().join("sub")).unwrap();
        std::fs::write(d.path().join("sub").join("d.mov"), b"x").unwrap();
        let mut f: Vec<_> = discover_files(d.path(), 4).into_iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        f.sort();
        assert_eq!(f, vec!["a.MP4", "d.mov"]);
    }

    #[tokio::test]
    async fn viewer_filter_hides_private_and_sources() {
        let db = Db::open_in_memory().unwrap();
        let info = crate::probe::MediaInfo { has_video: true, duration_ms: Some(1000), ..Default::default() };
        for (kind, vis, p) in [("source", "private", "/a.mp4"), ("export", "private", "/b.mp4"), ("export", "viewers", "/c.mp4")] {
            upsert(
                &db,
                NewAsset {
                    kind,
                    title: p.trim_start_matches('/').trim_end_matches(".mp4").into(),
                    path: Path::new(p),
                    size: 1,
                    mtime: None,
                    info: &info,
                    visibility: vis,
                    job_id: None,
                },
            )
            .await
            .unwrap();
        }
        let v = list(&db, ListFilter { viewer_only: true, ..Default::default() }).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].file_name, "c.mp4");
        assert_eq!(list(&db, ListFilter::default()).await.unwrap().len(), 3);
        let json = serde_json::to_string(&v[0]).unwrap();
        assert!(!json.contains("filePath") && !json.contains("thumbnailPath"));
        assert!(!json.contains("/c.mp4"), "không lộ đường dẫn đĩa");
    }
}

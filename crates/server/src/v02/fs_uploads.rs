//! P7: Duyệt thư mục máy chủ an toàn (`/api/fs/dirs`, `/api/fs/mkdir`) và
//! tải ảnh/video từ điện thoại / App Photos (`/api/videos/upload`, `/api/uploads`).

use std::path::{Path as FsPath, PathBuf};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::stream;

use super::{audit_ev, effective_upload_dir, St, MAX_MEDIA_UPLOAD_BYTES};

const UPLOAD_VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm", "avi"];
const UPLOAD_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "heic", "heif"];

// ======================= P7: Host Directory Browser =======================

#[derive(Deserialize)]
pub(crate) struct DirQuery {
    path: Option<String>,
}

pub(crate) async fn list_host_dirs(State(st): St, a: Auth, Query(q): Query<DirQuery>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let raw = q.path.unwrap_or_default();
    let trimmed = raw.trim().to_string();
    if !trimmed.is_empty() {
        if trimmed.contains('\0') {
            return Err(ApiError::validation("đường dẫn không hợp lệ"));
        }
        let p = FsPath::new(&trimmed);
        if !p.is_absolute() {
            return Err(ApiError::validation("cần đường dẫn tuyệt đối trên máy chủ"));
        }
    }
    let data_dir = st.paths.data_dir.clone();
    let res = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let mut roots = vec![];
        let disks = sysinfo::Disks::new_with_refreshed_list();
        for d in disks.list() {
            let mp = d.mount_point().to_string_lossy().into_owned();
            if !roots.iter().any(|(_, p): &(String, String)| p == &mp) {
                roots.push((format!("Ổ đĩa {mp}"), mp));
            }
        }
        if let Some(home) = doto_core::paths::user_home_dir() {
            for (lbl, sub) in [("Thư mục cá nhân", ""), ("Videos", "Videos"), ("Movies", "Movies"), ("Downloads", "Downloads"), ("Desktop", "Desktop")] {
                let cand = if sub.is_empty() { home.clone() } else { home.join(sub) };
                if cand.is_dir() {
                    roots.push((lbl.to_string(), cand.to_string_lossy().into_owned()));
                }
            }
        }
        roots.push(("Dữ liệu Doto Skill".into(), data_dir.to_string_lossy().into_owned()));
        let roots_json: Vec<Value> = roots.into_iter().map(|(label, path)| json!({ "label": label, "path": path })).collect();

        if trimmed.is_empty() {
            return Ok(json!({ "current": null, "parent": null, "roots": roots_json, "entries": [] }));
        }
        let target = PathBuf::from(&trimmed);
        if !target.is_dir() {
            return Err(format!("thư mục không tồn tại: {trimmed}"));
        }
        let canon = doto_core::paths::dunce_canonical(&target).unwrap_or(target);
        let parent = canon.parent().filter(|p| !p.as_os_str().is_empty() && *p != canon).map(|p| p.to_string_lossy().into_owned());
        let rd = std::fs::read_dir(&canon).map_err(|e| format!("không đọc được thư mục: {e}"))?;
        let mut entries: Vec<(String, String)> = rd
            .filter_map(Result::ok)
            .filter_map(|e| {
                let ft = e.file_type().ok()?;
                if !ft.is_dir() || ft.is_symlink() {
                    return None;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("System Volume Information") {
                    return None;
                }
                Some((name, e.path().to_string_lossy().into_owned()))
            })
            .collect();
        entries.sort_by_key(|a| a.0.to_ascii_lowercase());
        entries.truncate(300);
        let entries_json: Vec<Value> = entries.into_iter().map(|(name, path)| json!({ "name": name, "path": path })).collect();
        Ok(json!({
            "current": canon.to_string_lossy(),
            "parent": parent,
            "roots": roots_json,
            "entries": entries_json,
        }))
    })
    .await
    .map_err(anyhow::Error::from)?
    .map_err(ApiError::validation)?;

    Ok(Json(res))
}

#[derive(Deserialize)]
pub(crate) struct MkdirReq {
    parent: String,
    name: String,
    confirm: bool,
}

pub(crate) async fn create_host_dir(State(st): St, a: Auth, Json(b): Json<MkdirReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi tạo thư mục mới"));
    }
    let parent = PathBuf::from(b.parent.trim());
    if !parent.is_absolute() || !parent.is_dir() {
        return Err(ApiError::validation("thư mục cha phải là đường dẫn tuyệt đối đang tồn tại"));
    }
    let name = b.name.trim();
    if !doto_core::paths::is_safe_single_segment(name, 80) {
        return Err(ApiError::validation("tên thư mục mới không hợp lệ"));
    }
    let created = parent.join(name);
    tokio::fs::create_dir_all(&created).await.map_err(|e| ApiError::validation(format!("không tạo được thư mục: {e}")))?;
    let canon = doto_core::paths::dunce_canonical(&created).unwrap_or(created);
    let path_str = canon.to_string_lossy().into_owned();
    audit_ev(&st, &a, "fs.mkdir", Some(path_str.clone()), None).await;
    Ok(Json(json!({ "ok": true, "path": path_str })))
}

// ======================= P7: Mobile / App Photos Upload =======================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedMediaDto {
    pub id: String,
    pub original_name: String,
    pub safe_name: String,
    pub mime_type: String,
    pub media_kind: String,
    pub size_bytes: i64,
    pub video_id: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct UploadQuery {
    filename: Option<String>,
    title: Option<String>,
}

fn classify_upload_ext(filename: &str) -> Result<(&'static str, String, String), String> {
    let p = FsPath::new(filename);
    let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "media".into());
    let safe_stem = doto_media::library::sanitize_file_stem(&stem).replace(' ', "-");
    if UPLOAD_VIDEO_EXTS.contains(&ext.as_str()) {
        let mime = mime_guess::from_ext(&ext).first_or_octet_stream().essence_str().to_string();
        Ok(("video", safe_stem, if mime == "application/octet-stream" { "video/mp4".into() } else { mime }))
    } else if UPLOAD_IMAGE_EXTS.contains(&ext.as_str()) {
        let mime = mime_guess::from_ext(&ext).first_or_octet_stream().essence_str().to_string();
        Ok(("image", safe_stem, if mime == "application/octet-stream" { "image/jpeg".into() } else { mime }))
    } else {
        Err("chỉ hỗ trợ tệp video (.mp4, .mov, .m4v, .mkv, .webm, .avi) hoặc ảnh (.jpg, .png, .webp, .gif, .heic)".into())
    }
}

fn ensure_free_disk(dir: &FsPath, min_bytes: u64) -> Result<(), ApiError> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let canon = doto_core::paths::dunce_canonical(dir).unwrap_or_else(|| dir.to_path_buf());
    if let Some(d) = disks.list().iter().filter(|d| canon.starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len()) {
        if d.available_space() < min_bytes {
            return Err(ApiError::validation(format!("ổ đĩa máy chủ không đủ dung lượng trống (cần tối thiểu {} MB)", min_bytes / 1024 / 1024)));
        }
    }
    Ok(())
}

pub(crate) async fn upload_media(State(st): St, a: Auth, headers: HeaderMap, Query(q): Query<UploadQuery>, body: Body) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let raw_name = q.filename.or_else(|| headers.get("x-file-name").and_then(|v| v.to_str().ok()).map(String::from)).unwrap_or_default();
    let orig_name = FsPath::new(raw_name.trim()).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if orig_name.is_empty() {
        return Err(ApiError::validation("thiếu tên tệp tải lên (`?filename=...`)"));
    }
    let (media_kind, safe_stem, mime_type) = classify_upload_ext(&orig_name).map_err(ApiError::validation)?;
    let ext = FsPath::new(&orig_name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "mp4".into());

    let root = effective_upload_dir(&st).await;
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let day_dir = root.join(&day);
    tokio::fs::create_dir_all(&day_dir).await.map_err(|e| ApiError::validation(format!("không tạo được thư mục lưu media: {e}")))?;
    ensure_free_disk(&day_dir, 100 * 1024 * 1024)?;

    let id = doto_core::new_id();
    let safe_name = format!("{}-{}.{}", safe_stem, &id[..8], ext);
    let part_path = day_dir.join(format!(".{safe_name}.part"));
    let final_path = day_dir.join(&safe_name);

    let mut file = tokio::fs::File::create(&part_path).await.map_err(ApiError::internal)?;
    let mut stream = body.into_data_stream();
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(b) => b,
            Err(e) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(ApiError::validation(format!("luồng tải lên bị gián đoạn: {e}")));
            }
        };
        total += bytes.len() as u64;
        if total > MAX_MEDIA_UPLOAD_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(ApiError::new(axum::http::StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", "tệp vượt giới hạn 1 GB mỗi lần tải"));
        }
        if let Err(e) = file.write_all(&bytes).await {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(ApiError::internal(e));
        }
    }
    if total == 0 {
        drop(file);
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(ApiError::validation("tệp tải lên rỗng"));
    }
    file.flush().await.map_err(ApiError::internal)?;
    drop(file);
    tokio::fs::rename(&part_path, &final_path).await.map_err(ApiError::internal)?;

    let title_override = q
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| FsPath::new(&orig_name).file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()));

    let mut indexed_video = None;
    if media_kind == "video" {
        if let Ok(Some(asset)) = st.media.index_uploaded_source(&final_path, title_override).await {
            indexed_video = Some(asset);
        }
    }

    let now = doto_core::now_rfc3339();
    let vid_id = indexed_video.as_ref().map(|v| v.id.clone());
    let (id2, on2, sn2, fp2, mt2, mk2, sz2, vid2, now2) = (
        id.clone(),
        orig_name.clone(),
        safe_name.clone(),
        final_path.to_string_lossy().into_owned(),
        mime_type.clone(),
        media_kind.to_string(),
        total as i64,
        vid_id.clone(),
        now.clone(),
    );
    st.db
        .call(move |c| {
            c.execute(
                "INSERT INTO uploaded_media(id,original_name,safe_name,file_path,mime_type,media_kind,size_bytes,video_id,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![id2, on2, sn2, fp2, mt2, mk2, sz2, vid2, now2],
            )?;
            Ok(())
        })
        .await?;

    let dto = UploadedMediaDto {
        id: id.clone(),
        original_name: orig_name,
        safe_name,
        mime_type,
        media_kind: media_kind.to_string(),
        size_bytes: total as i64,
        video_id: vid_id,
        created_at: now,
    };
    audit_ev(&st, &a, "media.upload", Some(id), Some(json!({ "kind": media_kind, "bytes": total, "indexedVideo": indexed_video.is_some() }))).await;
    Ok(Json(json!({ "upload": dto, "video": indexed_video })))
}

pub(crate) async fn list_uploads(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let list: Vec<UploadedMediaDto> = st
        .db
        .call(|c| {
            let mut s = c.prepare(
                "SELECT id,original_name,safe_name,mime_type,media_kind,size_bytes,video_id,created_at
                 FROM uploaded_media ORDER BY created_at DESC LIMIT 300",
            )?;
            let v = s
                .query_map([], |r| {
                    Ok(UploadedMediaDto {
                        id: r.get(0)?,
                        original_name: r.get(1)?,
                        safe_name: r.get(2)?,
                        mime_type: r.get(3)?,
                        media_kind: r.get(4)?,
                        size_bytes: r.get(5)?,
                        video_id: r.get(6)?,
                        created_at: r.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(list)))
}

pub(crate) async fn preview_upload(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    a.require_owner()?;
    let i = id.clone();
    let row: Option<(String, String)> = st
        .db
        .call(move |c| Ok(c.query_row("SELECT file_path, mime_type FROM uploaded_media WHERE id=?1", [i], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))
        .await?;
    let Some((fp, mime)) = row else {
        return Err(ApiError::not_found("không tìm thấy tệp đã tải lên"));
    };
    let p = PathBuf::from(&fp);
    if !p.is_file() {
        return Err(ApiError::not_found("tệp không còn trên đĩa"));
    }
    if mime.starts_with("video/") {
        return stream::serve_file(&p, &headers, stream::ServeOpts { download_name: None, bytes_per_sec: None, permit: None }).await;
    }
    let bytes = tokio::fs::read(&p).await.map_err(ApiError::internal)?;
    Ok(([(header::CONTENT_TYPE, mime.as_str()), (header::CACHE_CONTROL, "private, max-age=3600")], bytes).into_response())
}

pub(crate) async fn delete_upload(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let i = id.clone();
    let row: Option<(String, Option<String>)> = st
        .db
        .call(move |c| {
            let r = c.query_row("SELECT file_path, video_id FROM uploaded_media WHERE id=?1", [&i], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            if r.is_some() {
                c.execute("DELETE FROM uploaded_media WHERE id=?1", [&i])?;
            }
            Ok(r)
        })
        .await?;
    let Some((fp, vid)) = row else {
        return Err(ApiError::not_found("không tìm thấy tệp đã tải lên"));
    };
    if let Some(vid) = vid {
        let _ = doto_media::library::delete(&st.db, &vid).await;
    }
    let root = effective_upload_dir(&st).await;
    let file_path = PathBuf::from(&fp);
    if doto_core::paths::is_within(&root, &file_path) || doto_core::paths::is_within(&st.paths.default_uploads_dir(), &file_path) {
        let _ = tokio::fs::remove_file(&file_path).await;
    }
    audit_ev(&st, &a, "media.upload.delete", Some(id), None).await;
    Ok(Json(json!({ "ok": true })))
}

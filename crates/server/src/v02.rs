//! Các nhóm API mở rộng bản v0.2.0 (P7 → P11):
//! - P7: Duyệt thư mục máy chủ an toàn (`/api/fs/dirs`, `/api/fs/mkdir`) + Tải ảnh/video từ điện thoại / App Photos (`/api/videos/upload`, `/api/uploads`).
//! - P8: Import Skill từ tệp nén `.zip`/`.tar.gz` (`/api/skill-sources/archive`) & tải thư mục từ trình duyệt (`/api/skill-sources/upload-folder`).
//! - P9: AI Studio chạy đơn phiên & đa phiên song song Antigravity / Claude Code / Codex (`/api/studio/*`).
//! - P10: Trình quản lý CRUD đa Cloudflare Tunnel & tiến trình ngoài (`/api/tunnels/*`).
//! - P11: Trung tâm Bộ nhớ chung & Bàn giao 3 AI (`/api/memory/*`).
//!
//! Mọi route ở đây đều yêu cầu quyền Owner đã xác thực (`a.require_owner()?`), hoạt động đồng nhất 100%
//! giữa bản Desktop App và bản Web (chạy trên phần cứng của máy chủ bản App). Viewer bị chặn tuyệt đối `403`.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use base64::Engine;
use futures::StreamExt;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use doto_core::{audit, settings};
use doto_integrations::{
    studio::{PreparedSession, StudioRunRequest},
    ManagedTunnelReq,
};

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::{memory, stream, AppState};

type St = State<Arc<AppState>>;

pub const MAX_MEDIA_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB
pub const MAX_SKILL_ARCHIVE_BYTES: usize = 100 * 1024 * 1024; // 100 MiB

const UPLOAD_VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm", "avi"];
const UPLOAD_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "heic", "heif"];

/// Các route JSON chuẩn (nằm trong giới hạn body 2 MiB mặc định).
pub fn json_routes() -> Router<Arc<AppState>> {
    Router::new()
        // P7: Host directory browser & uploaded media management
        .route("/api/fs/dirs", get(list_host_dirs))
        .route("/api/fs/mkdir", post(create_host_dir))
        .route("/api/uploads", get(list_uploads))
        .route("/api/uploads/{id}/preview", get(preview_upload))
        .route("/api/uploads/{id}", delete(delete_upload))
        // P9: AI Studio
        .route("/api/studio/catalog", get(studio_catalog))
        .route("/api/studio/runs", get(studio_list_runs).post(studio_start_run))
        .route("/api/studio/runs/{id}", get(studio_get_run))
        .route("/api/studio/runs/{id}/cancel", post(studio_cancel_run))
        .route("/api/studio/sessions/{id}/cancel", post(studio_cancel_session))
        // P10: Multi-Tunnel CRUD
        .route("/api/tunnels", get(tunnels_overview).post(tunnels_create))
        .route("/api/tunnels/quick-web", post(tunnels_quick_web))
        .route("/api/tunnels/{id}", patch(tunnels_update).delete(tunnels_delete))
        .route("/api/tunnels/{id}/start", post(tunnels_start))
        .route("/api/tunnels/{id}/stop", post(tunnels_stop))
        .route("/api/tunnels/processes/{pid}/stop", post(tunnels_stop_external))
        // P11: Shared Memory & 3-Agent Handoff Center
        .route("/api/memory", get(memory_overview))
        .route("/api/memory/doc", get(memory_read_doc).put(memory_write_doc))
        .route("/api/memory/append", post(memory_append))
        .route("/api/memory/init", post(memory_init))
        .route("/api/memory/handoff", post(memory_record_handoff))
}

/// Các route tải tệp nhị phân/thư mục lớn (giới hạn riêng tới 1 GiB stream cho video, 100 MiB cho gói skill).
pub fn upload_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/videos/upload", post(upload_media))
        .route("/api/skill-sources/archive", post(import_skill_archive))
        .route("/api/skill-sources/upload-folder", post(import_skill_folder_upload))
}

async fn audit_ev(st: &AppState, a: &Auth, action: &str, target: Option<String>, detail: Option<Value>) {
    audit::record(&st.db, a.username.clone(), action, target, detail, a.ip.clone()).await;
}

async fn effective_upload_dir(st: &AppState) -> PathBuf {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    app.media_upload_dir.filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| st.paths.default_uploads_dir())
}

async fn effective_custom_skill_dir(st: &AppState) -> PathBuf {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    app.custom_skill_dir.filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| st.paths.default_custom_skills_dir())
}

async fn effective_memory_dir(st: &AppState) -> (PathBuf, bool) {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    memory::resolve_workspace_dir(app.workspace_memory_dir.as_deref(), &st.paths.default_workspace_memory_dir())
}

// ======================= P7: Host Directory Browser =======================

#[derive(Deserialize)]
struct DirQuery {
    path: Option<String>,
}

async fn list_host_dirs(State(st): St, a: Auth, Query(q): Query<DirQuery>) -> ApiResult<Json<Value>> {
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
        if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from) {
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
struct MkdirReq {
    parent: String,
    name: String,
    confirm: bool,
}

async fn create_host_dir(State(st): St, a: Auth, Json(b): Json<MkdirReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi tạo thư mục mới"));
    }
    let parent = PathBuf::from(b.parent.trim());
    if !parent.is_absolute() || !parent.is_dir() {
        return Err(ApiError::validation("thư mục cha phải là đường dẫn tuyệt đối đang tồn tại"));
    }
    let name = b.name.trim();
    let valid_name = !name.is_empty()
        && name.chars().count() <= 80
        && name != "."
        && name != ".."
        && !name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'));
    if !valid_name {
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
struct UploadQuery {
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

async fn upload_media(State(st): St, a: Auth, headers: HeaderMap, Query(q): Query<UploadQuery>, body: Body) -> ApiResult<Json<Value>> {
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

async fn list_uploads(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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

async fn preview_upload(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
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

async fn delete_upload(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
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

// ======================= P8: Skill Archive & Browser Folder Import =======================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportQuery {
    filename: String,
    overwrite: Option<bool>,
    auto_md: Option<bool>,
    confirm: Option<bool>,
}

async fn import_skill_archive(State(st): St, a: Auth, Query(q): Query<ArchiveImportQuery>, body: Bytes) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !q.confirm.unwrap_or(false) {
        return Err(ApiError::validation("cần xác nhận trước khi giải nén và nhập gói skill"));
    }
    if body.is_empty() {
        return Err(ApiError::validation("tệp nén rỗng"));
    }
    if body.len() > MAX_SKILL_ARCHIVE_BYTES {
        return Err(ApiError::new(axum::http::StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", "tệp nén vượt giới hạn 100 MB"));
    }
    let fname = FsPath::new(q.filename.trim()).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if doto_skills::importer::archive_kind(&fname).is_none() {
        return Err(ApiError::validation("định dạng không hỗ trợ — chọn .zip, .tar.gz, .tgz, .tar, .7z hoặc .rar"));
    }

    let tmp = tempfile::Builder::new().prefix("doto-skill-arc-").tempdir_in(st.paths.tmp_dir()).map_err(ApiError::internal)?;
    let arc_file = tmp.path().join(&fname);
    let stage_dir = tmp.path().join("extracted");
    tokio::fs::write(&arc_file, &body).await.map_err(ApiError::internal)?;

    let af = arc_file.clone();
    let fn2 = fname.clone();
    let sd = stage_dir.clone();
    let extracted = tokio::task::spawn_blocking(move || doto_skills::importer::extract_archive(&af, &fn2, &sd))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| ApiError::validation(e.to_string()))?;

    let target_root = effective_custom_skill_dir(&st).await;
    let fallback_name = doto_skills::importer::archive_stem(&fname);
    let report =
        st.skills.import_staged(stage_dir, target_root, fallback_name, q.overwrite.unwrap_or(false), q.auto_md.unwrap_or(false), extracted.skipped).await?;

    audit_ev(&st, &a, "skills.import.archive", Some(fname), Some(json!({ "imported": report.imported.len(), "skipped": report.skipped.len() }))).await;
    Ok(Json(json!(report)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadedSkillFile {
    path: String,
    content_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderUploadReq {
    folder_name: String,
    files: Vec<UploadedSkillFile>,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    auto_md: bool,
    pub confirm: bool,
}

async fn import_skill_folder_upload(State(st): St, a: Auth, Json(b): Json<FolderUploadReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi tải thư mục skill lên máy chủ"));
    }
    if b.files.is_empty() {
        return Err(ApiError::validation("thư mục tải lên không có tệp nào"));
    }
    if b.files.len() > doto_skills::importer::MAX_UPLOAD_FILES {
        return Err(ApiError::validation(format!("tối đa {} tệp mỗi lần tải", doto_skills::importer::MAX_UPLOAD_FILES)));
    }
    let mut decoded = Vec::with_capacity(b.files.len());
    for f in b.files {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(f.content_base64.trim())
            .map_err(|_| ApiError::validation(format!("dữ liệu base64 không hợp lệ: {}", f.path)))?;
        decoded.push((f.path, bytes));
    }

    let tmp = tempfile::Builder::new().prefix("doto-skill-up-").tempdir_in(st.paths.tmp_dir()).map_err(ApiError::internal)?;
    let stage_dir = tmp.path().join("folder");
    let sd = stage_dir.clone();
    tokio::task::spawn_blocking(move || doto_skills::importer::write_uploaded_files(&sd, &decoded)).await.map_err(anyhow::Error::from)??;

    let target_root = effective_custom_skill_dir(&st).await;
    let fallback = if b.folder_name.trim().is_empty() { "uploaded-skill".into() } else { b.folder_name.trim().to_string() };
    let report = st.skills.import_staged(stage_dir, target_root, fallback.clone(), b.overwrite, b.auto_md, vec![]).await?;

    audit_ev(&st, &a, "skills.import.upload_folder", Some(fallback), Some(json!({ "imported": report.imported.len(), "skipped": report.skipped.len() }))).await;
    Ok(Json(json!(report)))
}

// ======================= P9: AI Studio =======================

async fn studio_catalog(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let providers = st.studio.catalog().await;
    Ok(Json(json!({ "providers": providers })))
}

async fn studio_list_runs(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.studio.list_runs(50).await?)))
}

async fn studio_get_run(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let r = st.studio.get_run(&id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy lượt chạy AI Studio"))?;
    Ok(Json(json!(r)))
}

async fn studio_start_run(State(st): St, a: Auth, Json(req): Json<StudioRunRequest>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let specs = doto_integrations::StudioRunner::expand_specs(&req).map_err(ApiError::validation)?;

    // Xác minh mọi provider được chọn đều đã có binary trên máy chủ
    let mut binaries = Vec::with_capacity(specs.len());
    for s in &specs {
        let bin = st
            .studio
            .locate_provider(&s.provider)
            .ok_or_else(|| ApiError::conflict(format!("chưa cài đặt công cụ `{}` trên máy chủ — vào mục Công cụ AI để cài đặt", s.provider)))?;
        binaries.push((s.clone(), bin));
    }

    // Xác minh skill (nếu chọn)
    let skill_resolved = match req.skill_id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(sid) => Some(st.skills.skill_dir_by_id(sid).await?),
        None => None,
    };

    // Xác minh danh sách media đính kèm từ `uploaded_media`
    let mut attachment_paths: Vec<(String, String)> = vec![];
    for uid in req.upload_ids.iter().take(20) {
        let u2 = uid.clone();
        let row: Option<(String, String)> = st
            .db
            .call(move |c| {
                Ok(c.query_row("SELECT original_name, file_path FROM uploaded_media WHERE id=?1", [u2], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
            })
            .await?;
        if let Some((orig, fp)) = row {
            if FsPath::new(&fp).is_file() {
                attachment_paths.push((orig, fp));
            }
        }
    }

    let base_out = st.media.output_dir().await;
    tokio::fs::create_dir_all(&base_out).await.map_err(|e| ApiError::validation(format!("không tạo được thư mục xuất: {e}")))?;
    let batch_tag = &doto_core::new_id()[..8];
    let (mem_dir, _) = effective_memory_dir(&st).await;

    let mut prepared = Vec::with_capacity(binaries.len());
    for (idx, (spec, binary)) in binaries.into_iter().enumerate() {
        let out_sub = base_out.join(format!("studio-{batch_tag}-s{}", idx + 1));
        let mut full_prompt = String::new();
        if let Some((sk, _)) = &skill_resolved {
            let prefix = if spec.provider == "codex" { "$" } else { "/" };
            full_prompt.push_str(&format!("{prefix}{} ", sk.canonical_id));
        }
        full_prompt.push_str(req.prompt.trim());
        full_prompt.push_str(&format!("\n\n[Thư mục xuất riêng cho phiên này]: {}", out_sub.display()));
        if !attachment_paths.is_empty() {
            full_prompt.push_str("\n[Tệp media người dùng đính kèm trên máy chủ]:");
            for (orig, fp) in &attachment_paths {
                full_prompt.push_str(&format!("\n- {orig}: {fp}"));
            }
        }
        if req.include_memory_hint {
            full_prompt.push_str(&format!(
                "\n[Bộ nhớ chung đa AI]: tham khảo PROJECT_MEMORY.md / MEMORY.md / HANDOFF.md tại {} nếu cần ngữ cảnh liên phiên.",
                mem_dir.display()
            ));
        }
        let working_dir = skill_resolved.as_ref().map(|(_, d)| d.clone()).unwrap_or_else(|| out_sub.clone());
        prepared.push(PreparedSession {
            spec,
            binary,
            working_dir,
            skill_dir: skill_resolved.as_ref().map(|(_, d)| d.clone()),
            output_subdir: out_sub,
            full_prompt,
        });
    }

    let title = req.title.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| {
        let snippet: String = req.prompt.trim().chars().take(48).collect();
        format!("Studio ({} phiên): {}", prepared.len(), snippet)
    });
    let vis = if req.visibility.as_deref() == Some("viewers") { "viewers".to_string() } else { "private".to_string() };

    let media_svc = st.media.clone();
    let hook: doto_integrations::studio::PostSessionHook = Arc::new(move |job_id, out_dir, visibility| {
        let m = media_svc.clone();
        Box::pin(async move {
            m.scan_directory_as_exports(&out_dir, &visibility, Some(job_id)).await.map(|v| v.into_iter().map(|a| a.id).collect()).unwrap_or_default()
        })
    });

    let detail = st.studio.launch(title, skill_resolved.map(|(s, _)| s.id), vis, prepared, Some(hook)).await?;

    audit_ev(&st, &a, "studio.run", Some(detail.job.id.clone()), Some(json!({ "mode": req.mode, "sessions": detail.sessions.len() }))).await;
    Ok(Json(json!(detail)))
}

async fn studio_cancel_run(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let n = st.studio.cancel_job(&id).await?;
    audit_ev(&st, &a, "studio.cancel_run", Some(id), Some(json!({ "canceledSessions": n }))).await;
    Ok(Json(json!({ "ok": true, "canceled": n })))
}

async fn studio_cancel_session(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let ok_cancel = st.studio.cancel_session(&id).await;
    audit_ev(&st, &a, "studio.cancel_session", Some(id), Some(json!({ "canceled": ok_cancel }))).await;
    Ok(Json(json!({ "ok": ok_cancel })))
}

// ======================= P10: Multi-Tunnel CRUD =======================

async fn tunnels_overview(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let port = st.public.local_port().unwrap_or(st.server_config().await.port);
    let ov = st.tunnel.overview(st.cloudflared_path(), port).await?;
    Ok(Json(json!(ov)))
}

async fn tunnels_create(State(st): St, a: Auth, Json(req): Json<ManagedTunnelReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let created = st.tunnel.create_managed(req).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.create", Some(created.id.clone()), Some(json!({ "kind": created.kind, "name": created.name }))).await;
    Ok(Json(json!(created)))
}

async fn tunnels_update(State(st): St, a: Auth, Path(id): Path<String>, Json(req): Json<ManagedTunnelReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let updated = st.tunnel.update_managed(&id, req).await.map_err(|e| {
        if e.to_string() == "NOT_FOUND" {
            ApiError::not_found("không tìm thấy cấu hình tunnel")
        } else {
            ApiError::validation(e.to_string())
        }
    })?;
    audit_ev(&st, &a, "tunnels.update", Some(id), Some(json!({ "kind": updated.kind, "name": updated.name }))).await;
    Ok(Json(json!(updated)))
}

async fn tunnels_delete(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !st.tunnel.delete_managed(&id).await? {
        return Err(ApiError::not_found("không tìm thấy cấu hình tunnel"));
    }
    audit_ev(&st, &a, "tunnels.delete", Some(id), None).await;
    Ok(Json(json!({ "ok": true })))
}

async fn tunnels_start(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !crate::auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("cần đặt mật khẩu Owner trước khi mở tunnel ra Internet"));
    }
    let bin = st.cloudflared_path().ok_or_else(|| ApiError::conflict("chưa cài cloudflared — vào Công cụ AI để cài"))?;
    let started = st.tunnel.start_managed(&id, bin).await.map_err(|e| ApiError::conflict(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.start", Some(id), Some(json!({ "kind": started.kind }))).await;
    Ok(Json(json!(started)))
}

async fn tunnels_stop(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.stop_managed(&id).await;
    let cur = st.tunnel.get_managed(&id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy tunnel"))?;
    audit_ev(&st, &a, "tunnels.stop", Some(id), None).await;
    Ok(Json(json!(cur)))
}

#[derive(Deserialize)]
struct ConfirmBody {
    confirm: bool,
}

async fn tunnels_quick_web(State(st): St, a: Auth, Json(b): Json<ConfirmBody>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi mở Quick Tunnel ra Internet"));
    }
    if !crate::auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("cần đặt mật khẩu Owner trước khi mở tunnel ra Internet"));
    }
    let bin = st.cloudflared_path().ok_or_else(|| ApiError::conflict("chưa cài cloudflared — vào Công cụ AI để cài"))?;
    let port = st.public.local_port().ok_or_else(|| ApiError::conflict("bật máy chủ web (Localhost hoặc LAN) trước khi mở Quick Tunnel"))?;
    let started = st.tunnel.start_quick_web(bin, port).await.map_err(|e| ApiError::conflict(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.quick_web", Some(started.id.clone()), Some(json!({ "port": port }))).await;
    Ok(Json(json!(started)))
}

async fn tunnels_stop_external(State(st): St, a: Auth, Path(pid): Path<u32>, Json(b): Json<ConfirmBody>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi dừng tiến trình cloudflared bên ngoài"));
    }
    st.tunnel.stop_external(pid).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.stop_external", Some(pid.to_string()), None).await;
    Ok(Json(json!({ "ok": true, "pid": pid })))
}

// ======================= P11: Shared Memory & 3-Agent Handoff Center =======================

async fn memory_overview(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let (dir, is_custom) = effective_memory_dir(&st).await;
    let d2 = dir.clone();
    let docs = tokio::task::spawn_blocking(move || memory::scan_docs(&d2)).await.map_err(anyhow::Error::from)?;
    let recent_handoffs: Vec<memory::HandoffEntry> = st
        .db
        .call(|c| {
            let mut s = c.prepare("SELECT id, agent, repo_sha, summary, created_at FROM handoffs ORDER BY id DESC LIMIT 30")?;
            let v = s
                .query_map([], |r| {
                    Ok(memory::HandoffEntry { id: r.get(0)?, agent: r.get(1)?, repo_sha: r.get(2)?, summary: r.get(3)?, created_at: r.get(4)? })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(memory::MemoryOverview { workspace_dir: dir.to_string_lossy().into_owned(), is_custom_dir: is_custom, docs, recent_handoffs })))
}

#[derive(Deserialize)]
struct DocQuery {
    id: String,
}

async fn memory_read_doc(State(st): St, a: Auth, Query(q): Query<DocQuery>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let (dir, _) = effective_memory_dir(&st).await;
    let doc_id = q.id.clone();
    let (spec, exists, content) =
        tokio::task::spawn_blocking(move || memory::read_doc(&dir, &doc_id)).await.map_err(anyhow::Error::from)?.map_err(ApiError::validation)?;
    Ok(Json(json!({ "id": spec.id, "relPath": spec.rel_path, "exists": exists, "content": content })))
}

#[derive(Deserialize)]
struct WriteDocReq {
    id: String,
    content: String,
    confirm: bool,
}

async fn memory_write_doc(State(st): St, a: Auth, Json(b): Json<WriteDocReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi lưu tệp bộ nhớ chung"));
    }
    let (dir, _) = effective_memory_dir(&st).await;
    let doc_id = b.id.clone();
    let content = b.content;
    let spec =
        tokio::task::spawn_blocking(move || memory::write_doc(&dir, &doc_id, &content)).await.map_err(anyhow::Error::from)?.map_err(ApiError::validation)?;
    audit_ev(&st, &a, "memory.write", Some(spec.id.to_string()), None).await;
    Ok(Json(json!({ "ok": true, "id": spec.id })))
}

#[derive(Deserialize)]
struct AppendNoteReq {
    id: Option<String>,
    agent: Option<String>,
    note: String,
}

async fn memory_append(State(st): St, a: Auth, Json(b): Json<AppendNoteReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let (dir, _) = effective_memory_dir(&st).await;
    let target = b.id.unwrap_or_else(|| "MEMORY.md".into());
    let agent = b.agent.unwrap_or_else(|| a.username.clone());
    let note = b.note;
    let spec = tokio::task::spawn_blocking(move || memory::append_note(&dir, &target, &agent, &note))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(ApiError::validation)?;
    audit_ev(&st, &a, "memory.append", Some(spec.id.to_string()), None).await;
    Ok(Json(json!({ "ok": true, "id": spec.id })))
}

#[derive(Deserialize)]
struct InitMemoryReq {
    #[serde(default)]
    overwrite: bool,
    confirm: bool,
}

async fn memory_init(State(st): St, a: Auth, Json(b): Json<InitMemoryReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi khởi tạo bộ tệp bàn giao"));
    }
    let (dir, _) = effective_memory_dir(&st).await;
    let (created, kept) = tokio::task::spawn_blocking(move || memory::init_missing_templates(&dir, b.overwrite))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(ApiError::validation)?;
    audit_ev(&st, &a, "memory.init", None, Some(json!({ "created": created.len(), "overwrite": b.overwrite }))).await;
    Ok(Json(json!({ "ok": true, "created": created, "kept": kept })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordHandoffReq {
    agent: String,
    summary: String,
    next_action: Option<String>,
}

async fn memory_record_handoff(State(st): St, a: Auth, Json(b): Json<RecordHandoffReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !matches!(b.agent.as_str(), "antigravity" | "claude" | "codex") {
        return Err(ApiError::validation("agent phải là antigravity, claude hoặc codex"));
    }
    let clean_summary = doto_core::redact_secrets(b.summary.trim());
    if clean_summary.is_empty() || clean_summary.chars().count() > 2000 {
        return Err(ApiError::validation("tóm tắt bàn giao phải từ 1 đến 2000 ký tự"));
    }
    let clean_next = b.next_action.as_deref().map(|s| doto_core::redact_secrets(s.trim())).filter(|s| !s.is_empty());
    let full = match &clean_next {
        Some(nx) => format!("{clean_summary} · NEXT: {nx}"),
        None => clean_summary.clone(),
    };

    let (dir, _) = effective_memory_dir(&st).await;
    let d2 = dir.clone();
    let ag2 = b.agent.clone();
    let f2 = full.clone();
    let _ = tokio::task::spawn_blocking(move || memory::append_note(&d2, "HANDOFF.md", &ag2, &f2)).await;

    let (ag3, f3, now) = (b.agent.clone(), full.clone(), doto_core::now_rfc3339());
    st.db
        .call(move |c| {
            c.execute("INSERT INTO handoffs(agent, repo_sha, summary, created_at) VALUES(?1, NULL, ?2, ?3)", params![ag3, f3, now])?;
            Ok(())
        })
        .await?;
    audit_ev(&st, &a, "memory.handoff", Some(b.agent), None).await;
    Ok(Json(json!({ "ok": true })))
}

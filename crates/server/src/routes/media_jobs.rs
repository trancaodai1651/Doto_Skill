//! Route thư viện video nguồn/xuất, HTTP Range streaming, tải xuống, chia sẻ có thời hạn
//! và hàng đợi job render FFmpeg / AI Studio.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_core::jobs;
use doto_media::library;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::{stream, AppState};

use super::{audit_ev, ok, LimitQ, St};

// ======================= Video =======================

#[derive(Deserialize)]
pub(crate) struct VideoQ {
    kind: Option<String>,
    q: Option<String>,
}

fn can_view(a: &Auth, v: &library::VideoAsset) -> bool {
    a.is_owner() || (v.kind == "export" && v.visibility == "viewers" && v.status == "ready")
}

/// Lấy video + kiểm tra quyền; không có quyền → 404 (không tiết lộ tồn tại, chống IDOR).
async fn visible_video(st: &AppState, a: &Auth, id: &str) -> ApiResult<library::VideoAsset> {
    let v = library::get(&st.db, id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy video"))?;
    if !can_view(a, &v) {
        return Err(ApiError::not_found("không tìm thấy video"));
    }
    Ok(v)
}

pub(crate) async fn list_videos(State(st): St, a: Auth, Query(q): Query<VideoQ>) -> ApiResult<Json<Value>> {
    let v = library::list(&st.db, library::ListFilter { kind: q.kind, q: q.q, viewer_only: !a.is_owner() }).await?;
    Ok(Json(json!(v)))
}

pub(crate) async fn get_video(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(visible_video(&st, &a, &id).await?)))
}

#[derive(Deserialize)]
pub(crate) struct VideoPatch {
    title: Option<String>,
    visibility: Option<String>,
}

pub(crate) async fn patch_video(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<VideoPatch>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    if b.visibility.as_deref() == Some("viewers") && v.kind != "export" {
        return Err(ApiError::validation("chỉ video đã xuất mới chia sẻ cho Viewer"));
    }
    if !library::update_meta(&st.db, &id, b.title, b.visibility.clone()).await.map_err(|e| ApiError::validation(e.to_string()))? {
        return Err(ApiError::not_found("không tìm thấy video"));
    }
    let v = visible_video(&st, &a, &id).await?;
    st.events.emit_scoped("video", &v, v.visibility == "viewers");
    audit_ev(&st, &a, "video.update", Some(id), Some(json!({ "visibility": b.visibility }))).await;
    Ok(Json(json!(v)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteQ {
    delete_file: Option<bool>,
}

pub(crate) async fn delete_video(State(st): St, a: Auth, Path(id): Path<String>, Query(q): Query<DeleteQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    let delete_file = q.delete_file.unwrap_or(false);
    if delete_file && v.kind != "export" {
        return Err(ApiError::validation("không xoá file video nguồn của người dùng — chỉ gỡ khỏi thư viện"));
    }
    library::delete(&st.db, &id).await?;
    if let Some(t) = &v.thumbnail_path {
        let _ = tokio::fs::remove_file(t).await;
    }
    if delete_file {
        let _ = tokio::fs::remove_file(&v.file_path).await;
    }
    audit_ev(&st, &a, "video.delete", Some(id), Some(json!({ "deleteFile": delete_file, "kind": v.kind }))).await;
    Ok(ok())
}

pub(crate) async fn video_thumbnail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let p = v.thumbnail_path.filter(|p| p.is_file()).ok_or_else(|| ApiError::not_found("chưa có thumbnail"))?;
    let bytes = tokio::fs::read(p).await.map_err(ApiError::internal)?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=3600")], bytes).into_response())
}

async fn stream_permit(st: &AppState) -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    let total = 24usize;
    let in_use = total - st.stream_slots.available_permits();
    if in_use >= st.stream_limit() {
        return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "busy", "máy chủ đang phát nhiều luồng — thử lại sau"));
    }
    st.stream_slots.clone().try_acquire_owned().map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "busy", "máy chủ đang bận"))
}

async fn bytes_per_sec(st: &AppState) -> Option<u64> {
    let kbps = st.server_config().await.max_stream_kbps;
    (kbps > 0).then_some(kbps * 1000 / 8)
}

pub(crate) async fn video_stream(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let permit = stream_permit(&st).await?;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: None, bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) }).await
}

pub(crate) async fn video_download(State(st): St, a: Auth, Path(id): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let v = visible_video(&st, &a, &id).await?;
    let permit = stream_permit(&st).await?;
    let ext = v.file_path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "mp4".into());
    let name = format!("{}.{}", doto_media::library::sanitize_file_stem(&v.title), ext);
    audit_ev(&st, &a, "video.download", Some(id), None).await;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: Some(&name), bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) })
        .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareReq {
    ttl_minutes: Option<i64>,
}

pub(crate) async fn create_share(State(st): St, a: Auth, Path(id): Path<String>, Json(b): Json<ShareReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let v = visible_video(&st, &a, &id).await?;
    if v.kind != "export" {
        return Err(ApiError::validation("chỉ chia sẻ được video đã xuất"));
    }
    let ttl = b.ttl_minutes.unwrap_or(60).clamp(5, 7 * 24 * 60);
    let token = doto_core::random_token(24);
    let sid = doto_core::new_id();
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(ttl)).to_rfc3339();
    let (s2, th, vid, by, e2) = (sid.clone(), doto_core::sha256_hex(&token), id.clone(), a.user_id.clone(), expires.clone());
    st.db
        .call(move |c| {
            c.execute(
                "INSERT INTO share_links(id,token_hash,video_id,created_by,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![s2, th, vid, by, doto_core::now_rfc3339(), e2],
            )?;
            Ok(())
        })
        .await?;
    let cfg = st.server_config().await;
    let base = cfg.public_base_url.clone().or_else(|| st.public.status(&cfg).urls.into_iter().last()).unwrap_or_default();
    audit_ev(&st, &a, "share.create", Some(id.clone()), Some(json!({ "ttlMinutes": ttl }))).await;
    Ok(Json(json!({ "id": sid, "videoId": id, "url": format!("{}/s/{token}", base.trim_end_matches('/')), "expiresAt": expires })))
}

pub(crate) async fn list_shares(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let _ = visible_video(&st, &a, &id).await?;
    let now = doto_core::now_rfc3339();
    let vid = id.clone();
    let list: Vec<Value> = st
        .db
        .call(move |c| {
            let mut s = c.prepare(
                "SELECT id, video_id, created_at, expires_at FROM share_links WHERE video_id=?1 AND revoked=0 AND expires_at > ?2 ORDER BY created_at DESC",
            )?;
            let v = s
                .query_map(params![vid, now], |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?,
                        "videoId": r.get::<_, String>(1)?,
                        "url": null,
                        "createdAt": r.get::<_, String>(2)?,
                        "expiresAt": r.get::<_, String>(3)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(json!(list)))
}

pub(crate) async fn revoke_share(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let i = id.clone();
    let n = st.db.call(move |c| Ok(c.execute("UPDATE share_links SET revoked=1 WHERE id=?1", [i])?)).await?;
    if n == 0 {
        return Err(ApiError::not_found("không tìm thấy link"));
    }
    audit_ev(&st, &a, "share.revoke", Some(id), None).await;
    Ok(ok())
}

pub(crate) async fn share_stream(State(st): St, Path(token): Path<String>, headers: HeaderMap) -> ApiResult<Response> {
    let th = doto_core::sha256_hex(&token);
    let now = doto_core::now_rfc3339();
    let vid: Option<String> = st
        .db
        .call(move |c| {
            Ok(c.query_row("SELECT video_id FROM share_links WHERE token_hash=?1 AND revoked=0 AND expires_at > ?2", params![th, now], |r| r.get(0))
                .optional()?)
        })
        .await?;
    let vid = vid.ok_or_else(|| ApiError::not_found("link không tồn tại hoặc đã hết hạn"))?;
    let v = library::get(&st.db, &vid).await?.filter(|v| v.status == "ready").ok_or_else(|| ApiError::not_found("video không còn"))?;
    let permit = stream_permit(&st).await?;
    stream::serve_file(&v.file_path, &headers, stream::ServeOpts { download_name: None, bytes_per_sec: bytes_per_sec(&st).await, permit: Some(permit) }).await
}

pub(crate) async fn scan_videos(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let s = st.media.scan_sources().await?;
    audit_ev(&st, &a, "video.scan", None, Some(json!({ "added": s.added_or_updated, "missing": s.missing }))).await;
    Ok(Json(json!(s)))
}

// ======================= Jobs =======================

pub(crate) async fn list_jobs(State(st): St, a: Auth, Query(q): Query<LimitQ>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(jobs::list(&st.db, q.limit.unwrap_or(200)).await?)))
}

pub(crate) async fn job_detail(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(jobs::detail(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?)))
}

pub(crate) async fn create_render(State(st): St, a: Auth, Json(b): Json<doto_media::RenderRequest>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let job = st.media.submit_render(b).await?;
    audit_ev(&st, &a, "job.render", Some(job.id.clone()), None).await;
    Ok(Json(json!(job)))
}

pub(crate) async fn cancel_job(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let j = jobs::get(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?;
    if j.kind == "studio" {
        let n = st.studio.cancel_job(&id).await?;
        if n == 0 && !matches!(j.status.as_str(), "running" | "queued") {
            return Err(ApiError::conflict("job đã kết thúc"));
        }
        audit_ev(&st, &a, "job.cancel", Some(id), Some(json!({ "kind": "studio" }))).await;
        return Ok(ok());
    }
    if j.kind != "render" {
        return Err(ApiError::conflict("chỉ huỷ được job render hoặc AI Studio"));
    }
    if !st.media.cancel(&id).await? {
        return Err(ApiError::conflict("job đã kết thúc"));
    }
    audit_ev(&st, &a, "job.cancel", Some(id), None).await;
    Ok(ok())
}

pub(crate) async fn retry_job(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let j = jobs::get(&st.db, &id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy job"))?;
    if j.kind != "render" {
        return Err(ApiError::conflict("chỉ thử lại được job render"));
    }
    if !st.media.retry(&id).await? {
        return Err(ApiError::conflict("chỉ thử lại job thất bại hoặc đã huỷ"));
    }
    audit_ev(&st, &a, "job.retry", Some(id.clone()), None).await;
    Ok(Json(json!(jobs::get(&st.db, &id).await?)))
}

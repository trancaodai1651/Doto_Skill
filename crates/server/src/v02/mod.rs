//! Các nhóm API mở rộng bản v0.2.0 (P7 → P11), chia thành các module chuyên trách:
//! - [`fs_uploads`]: P7 — Duyệt thư mục máy chủ an toàn (`/api/fs/*`) & tải ảnh/video từ điện thoại / App Photos (`/api/videos/upload`, `/api/uploads`).
//! - [`skills_studio`]: P8 (Import gói skill `.zip`/`.tar.gz` & tải thư mục trình duyệt), P9 (AI Studio đơn/đa phiên `/api/studio/*`),
//!   P10 (CRUD đa Cloudflare Tunnel `/api/tunnels/*`), P11 (Trung tâm Bộ nhớ chung & Bàn giao 3 AI `/api/memory/*`).
//!
//! Mọi route đều yêu cầu quyền Owner đã xác thực (`a.require_owner()?`), hoạt động đồng nhất 100%
//! giữa bản Desktop App và bản Web (chạy trên phần cứng máy chủ bản App). Viewer bị chặn tuyệt đối `403`.

pub mod fs_uploads;
pub mod skills_studio;

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{delete, get, patch, post};
use axum::Router;
use doto_core::settings;

pub(crate) use crate::routes::audit_ev;
use crate::{memory, AppState};

pub(crate) type St = axum::extract::State<Arc<AppState>>;

pub const MAX_MEDIA_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB
pub const MAX_SKILL_ARCHIVE_BYTES: usize = 100 * 1024 * 1024; // 100 MiB

pub use fs_uploads::UploadedMediaDto;

/// Các route JSON chuẩn (nằm trong giới hạn body 2 MiB mặc định).
pub fn json_routes() -> Router<Arc<AppState>> {
    Router::new()
        // P7: Host directory browser & uploaded media management
        .route("/api/fs/dirs", get(fs_uploads::list_host_dirs))
        .route("/api/fs/mkdir", post(fs_uploads::create_host_dir))
        .route("/api/uploads", get(fs_uploads::list_uploads))
        .route("/api/uploads/{id}/preview", get(fs_uploads::preview_upload))
        .route("/api/uploads/{id}", delete(fs_uploads::delete_upload))
        // P9: AI Studio
        .route("/api/studio/catalog", get(skills_studio::studio_catalog))
        .route("/api/studio/runs", get(skills_studio::studio_list_runs).post(skills_studio::studio_start_run))
        .route("/api/studio/runs/{id}", get(skills_studio::studio_get_run))
        .route("/api/studio/runs/{id}/cancel", post(skills_studio::studio_cancel_run))
        .route("/api/studio/sessions/{id}/cancel", post(skills_studio::studio_cancel_session))
        // P10: Multi-Tunnel CRUD
        .route("/api/tunnels", get(skills_studio::tunnels_overview).post(skills_studio::tunnels_create))
        .route("/api/tunnels/quick-web", post(skills_studio::tunnels_quick_web))
        .route("/api/tunnels/{id}", patch(skills_studio::tunnels_update).delete(skills_studio::tunnels_delete))
        .route("/api/tunnels/{id}/start", post(skills_studio::tunnels_start))
        .route("/api/tunnels/{id}/stop", post(skills_studio::tunnels_stop))
        .route("/api/tunnels/processes/{pid}/stop", post(skills_studio::tunnels_stop_external))
        // P11: Shared Memory & 3-Agent Handoff Center
        .route("/api/memory", get(skills_studio::memory_overview))
        .route("/api/memory/doc", get(skills_studio::memory_read_doc).put(skills_studio::memory_write_doc))
        .route("/api/memory/append", post(skills_studio::memory_append))
        .route("/api/memory/init", post(skills_studio::memory_init))
        .route("/api/memory/handoff", post(skills_studio::memory_record_handoff))
}

/// Các route tải tệp nhị phân/thư mục lớn (giới hạn riêng tới 1 GiB stream cho video, 100 MiB cho gói skill).
pub fn upload_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/videos/upload", post(fs_uploads::upload_media))
        .route("/api/skill-sources/archive", post(skills_studio::import_skill_archive))
        .route("/api/skill-sources/upload-folder", post(skills_studio::import_skill_folder_upload))
}

pub(crate) async fn effective_upload_dir(st: &AppState) -> PathBuf {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    app.media_upload_dir.filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| st.paths.default_uploads_dir())
}

pub(crate) async fn effective_custom_skill_dir(st: &AppState) -> PathBuf {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    app.custom_skill_dir.filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| st.paths.default_custom_skills_dir())
}

pub(crate) async fn effective_memory_dir(st: &AppState) -> (PathBuf, bool) {
    let app: settings::AppSettings = settings::get(&st.db, settings::KEY_APP).await.unwrap_or_default();
    memory::resolve_workspace_dir(app.workspace_memory_dir.as_deref(), &st.paths.default_workspace_memory_dir())
}

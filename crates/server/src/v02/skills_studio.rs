//! P8 (Import gói skill `.zip`/`.tar.gz` & tải thư mục trình duyệt),
//! P9 (AI Studio đơn/đa phiên `/api/studio/*`),
//! P10 (CRUD đa Cloudflare Tunnel `/api/tunnels/*`),
//! P11 (Trung tâm Bộ nhớ chung & Bàn giao 3 AI `/api/memory/*`).

use std::path::Path as FsPath;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::Json;
use base64::Engine;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use doto_integrations::{
    studio::{PreparedSession, StudioRunRequest},
    ManagedTunnelReq,
};

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::memory;

use super::{audit_ev, effective_custom_skill_dir, effective_memory_dir, St, MAX_SKILL_ARCHIVE_BYTES};

// ======================= P8: Skill Archive & Browser Folder Import =======================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveImportQuery {
    filename: String,
    overwrite: Option<bool>,
    auto_md: Option<bool>,
    confirm: Option<bool>,
}

pub(crate) async fn import_skill_archive(State(st): St, a: Auth, Query(q): Query<ArchiveImportQuery>, body: Bytes) -> ApiResult<Json<Value>> {
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
pub(crate) struct UploadedSkillFile {
    path: String,
    content_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FolderUploadReq {
    folder_name: String,
    files: Vec<UploadedSkillFile>,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    auto_md: bool,
    pub confirm: bool,
}

pub(crate) async fn import_skill_folder_upload(State(st): St, a: Auth, Json(b): Json<FolderUploadReq>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn studio_catalog(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let providers = st.studio.catalog().await;
    Ok(Json(json!({ "providers": providers })))
}

pub(crate) async fn studio_list_runs(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    Ok(Json(json!(st.studio.list_runs(50).await?)))
}

pub(crate) async fn studio_get_run(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let r = st.studio.get_run(&id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy lượt chạy AI Studio"))?;
    Ok(Json(json!(r)))
}

pub(crate) async fn studio_start_run(State(st): St, a: Auth, Json(req): Json<StudioRunRequest>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let specs = doto_integrations::StudioRunner::expand_specs(&req).map_err(ApiError::validation)?;

    let mut binaries = Vec::with_capacity(specs.len());
    for s in &specs {
        let bin = st
            .studio
            .locate_provider(&s.provider)
            .ok_or_else(|| ApiError::conflict(format!("chưa cài đặt công cụ `{}` trên máy chủ — vào mục Công cụ AI để cài đặt", s.provider)))?;
        binaries.push((s.clone(), bin));
    }

    let skill_resolved = match req.skill_id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(sid) => Some(st.skills.skill_dir_by_id(sid).await?),
        None => None,
    };

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

pub(crate) async fn studio_cancel_run(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let n = st.studio.cancel_job(&id).await?;
    audit_ev(&st, &a, "studio.cancel_run", Some(id), Some(json!({ "canceledSessions": n }))).await;
    Ok(Json(json!({ "ok": true, "canceled": n })))
}

pub(crate) async fn studio_cancel_session(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let ok_cancel = st.studio.cancel_session(&id).await;
    audit_ev(&st, &a, "studio.cancel_session", Some(id), Some(json!({ "canceled": ok_cancel }))).await;
    Ok(Json(json!({ "ok": ok_cancel })))
}

// ======================= P10: Multi-Tunnel CRUD =======================

pub(crate) async fn tunnels_overview(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let port = st.public.local_port().unwrap_or(st.server_config().await.port);
    let ov = st.tunnel.overview(st.cloudflared_path(), port).await?;
    Ok(Json(json!(ov)))
}

pub(crate) async fn tunnels_create(State(st): St, a: Auth, Json(req): Json<ManagedTunnelReq>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let created = st.tunnel.create_managed(req).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.create", Some(created.id.clone()), Some(json!({ "kind": created.kind, "name": created.name }))).await;
    Ok(Json(json!(created)))
}

pub(crate) async fn tunnels_update(State(st): St, a: Auth, Path(id): Path<String>, Json(req): Json<ManagedTunnelReq>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn tunnels_delete(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !st.tunnel.delete_managed(&id).await? {
        return Err(ApiError::not_found("không tìm thấy cấu hình tunnel"));
    }
    audit_ev(&st, &a, "tunnels.delete", Some(id), None).await;
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn tunnels_start(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !crate::auth::owner_password_set(&st).await {
        return Err(ApiError::conflict("cần đặt mật khẩu Owner trước khi mở tunnel ra Internet"));
    }
    let bin = st.cloudflared_path().ok_or_else(|| ApiError::conflict("chưa cài cloudflared — vào Công cụ AI để cài"))?;
    let started = st.tunnel.start_managed(&id, bin).await.map_err(|e| ApiError::conflict(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.start", Some(id), Some(json!({ "kind": started.kind }))).await;
    Ok(Json(json!(started)))
}

pub(crate) async fn tunnels_stop(State(st): St, a: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    st.tunnel.stop_managed(&id).await;
    let cur = st.tunnel.get_managed(&id).await?.ok_or_else(|| ApiError::not_found("không tìm thấy tunnel"))?;
    audit_ev(&st, &a, "tunnels.stop", Some(id), None).await;
    Ok(Json(json!(cur)))
}

#[derive(Deserialize)]
pub(crate) struct ConfirmBody {
    confirm: bool,
}

pub(crate) async fn tunnels_quick_web(State(st): St, a: Auth, Json(b): Json<ConfirmBody>) -> ApiResult<Json<Value>> {
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

pub(crate) async fn tunnels_stop_external(State(st): St, a: Auth, Path(pid): Path<u32>, Json(b): Json<ConfirmBody>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    if !b.confirm {
        return Err(ApiError::validation("cần xác nhận trước khi dừng tiến trình cloudflared bên ngoài"));
    }
    st.tunnel.stop_external(pid).await.map_err(|e| ApiError::validation(e.to_string()))?;
    audit_ev(&st, &a, "tunnels.stop_external", Some(pid.to_string()), None).await;
    Ok(Json(json!({ "ok": true, "pid": pid })))
}

// ======================= P11: Shared Memory & 3-Agent Handoff Center =======================

pub(crate) async fn memory_overview(State(st): St, a: Auth) -> ApiResult<Json<Value>> {
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
pub(crate) struct DocQuery {
    id: String,
}

pub(crate) async fn memory_read_doc(State(st): St, a: Auth, Query(q): Query<DocQuery>) -> ApiResult<Json<Value>> {
    a.require_owner()?;
    let (dir, _) = effective_memory_dir(&st).await;
    let doc_id = q.id.clone();
    let (spec, exists, content) =
        tokio::task::spawn_blocking(move || memory::read_doc(&dir, &doc_id)).await.map_err(anyhow::Error::from)?.map_err(ApiError::validation)?;
    Ok(Json(json!({ "id": spec.id, "relPath": spec.rel_path, "exists": exists, "content": content })))
}

#[derive(Deserialize)]
pub(crate) struct WriteDocReq {
    id: String,
    content: String,
    confirm: bool,
}

pub(crate) async fn memory_write_doc(State(st): St, a: Auth, Json(b): Json<WriteDocReq>) -> ApiResult<Json<Value>> {
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
pub(crate) struct AppendNoteReq {
    id: Option<String>,
    agent: Option<String>,
    note: String,
}

pub(crate) async fn memory_append(State(st): St, a: Auth, Json(b): Json<AppendNoteReq>) -> ApiResult<Json<Value>> {
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
pub(crate) struct InitMemoryReq {
    #[serde(default)]
    overwrite: bool,
    confirm: bool,
}

pub(crate) async fn memory_init(State(st): St, a: Auth, Json(b): Json<InitMemoryReq>) -> ApiResult<Json<Value>> {
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
pub(crate) struct RecordHandoffReq {
    agent: String,
    summary: String,
    next_action: Option<String>,
}

pub(crate) async fn memory_record_handoff(State(st): St, a: Auth, Json(b): Json<RecordHandoffReq>) -> ApiResult<Json<Value>> {
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

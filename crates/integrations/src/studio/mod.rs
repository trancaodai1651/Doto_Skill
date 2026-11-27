//! AI Studio Runner: điều phối chạy đơn phiên & đa phiên song song trên phần cứng máy chủ
//! cho 3 công cụ AI CLI (`antigravity` / `agy`, `claude`, `codex`).
//!
//! Nguyên tắc an toàn:
//! - Không bao giờ ghép chuỗi qua shell; chỉ truyền mảng đối số cố định qua `tokio::process::Command`.
//! - Bắt buộc Owner xác nhận (`confirm = true`) ở tầng API.
//! - Mỗi phiên chạy có thư mục xuất cô lập (`<output_dir>/studio-<job8>-s<idx>`) để chạy song song không đụng độ.
//! - Toàn bộ stdout/stderr đều lọc bí mật (`redact_secrets`) trước khi lưu SQLite hoặc phát SSE.

pub mod cli;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use rusqlite::{params, OptionalExtension};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_util::sync::CancellationToken;

pub use cli::{
    build_invocation, default_antigravity_models, default_claude_models, default_codex_models, parse_stream_json_line, validate_provider_model_effort,
    StudioModelInfo, StudioProviderCatalog, StudioRunDetail, StudioRunRequest, StudioSession, StudioSessionSpec, MAX_PARALLEL_SESSIONS, MAX_PROMPT_CHARS,
};

use doto_core::{events::EventBus, jobs, Db};

use crate::manager::{command, run_fixed, ToolManager};
use crate::providers;

const MAX_SESSION_LOG_BYTES: usize = 64 * 1024;

pub type PostSessionHook = Arc<dyn Fn(String, PathBuf, String) -> futures_box::BoxFuture<'static, Vec<String>> + Send + Sync>;

mod futures_box {
    pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
}

pub struct PreparedSession {
    pub spec: StudioSessionSpec,
    pub binary: PathBuf,
    pub working_dir: PathBuf,
    pub skill_dir: Option<PathBuf>,
    pub output_subdir: PathBuf,
    pub full_prompt: String,
}

pub struct StudioRunner {
    db: Db,
    events: EventBus,
    tools: Arc<ToolManager>,
    cancels: Mutex<HashMap<String, CancellationToken>>,
    binary_overrides: Mutex<HashMap<String, PathBuf>>,
}

impl StudioRunner {
    pub fn new(db: Db, events: EventBus, tools: Arc<ToolManager>) -> Arc<Self> {
        Arc::new(Self { db, events, tools, cancels: Mutex::new(HashMap::new()), binary_overrides: Mutex::new(HashMap::new()) })
    }

    /// Chỉ dùng trong kiểm thử tự động để trỏ provider sang script giả lập an toàn.
    pub fn set_binary_override(&self, provider: &str, path: PathBuf) {
        self.binary_overrides.lock().unwrap().insert(provider.to_string(), path);
    }

    pub fn locate_provider(&self, provider: &str) -> Option<PathBuf> {
        if let Some(p) = self.binary_overrides.lock().unwrap().get(provider) {
            return Some(p.clone());
        }
        let p = providers::by_id(provider)?;
        self.tools.locate(p.as_ref())
    }

    pub async fn catalog(&self) -> Vec<StudioProviderCatalog> {
        let mut out = vec![];
        for (id, vendor, prefix, models) in [
            ("antigravity", "Google DeepMind", "/", default_antigravity_models()),
            ("claude", "Anthropic", "/", default_claude_models()),
            ("codex", "OpenAI", "$", default_codex_models()),
        ] {
            let p = providers::by_id(id).expect("provider");
            let bin = self.locate_provider(id);
            let ver = match &bin {
                Some(b) => run_fixed(b, &["--version"], std::time::Duration::from_secs(5))
                    .await
                    .and_then(|(_, o, e)| providers::parse_version(&o).or_else(|| providers::parse_version(&e))),
                None => None,
            };
            out.push(StudioProviderCatalog {
                id: id.into(),
                name: p.name().into(),
                vendor: vendor.into(),
                installed: bin.is_some(),
                binary_path: bin.map(|b| b.to_string_lossy().into_owned()),
                version: ver,
                skill_prefix: prefix,
                models,
            });
        }
        out
    }

    /// Chuẩn hoá yêu cầu `single` / `multi_same_ai` / `multi_ai` / `custom_batch` thành danh sách `StudioSessionSpec`.
    pub fn expand_specs(req: &StudioRunRequest) -> Result<Vec<StudioSessionSpec>, String> {
        if !req.confirm {
            return Err("cần xác nhận trước khi khởi chạy phiên AI trên máy chủ".into());
        }
        let prompt = req.prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err(format!("nội dung yêu cầu phải từ 1 đến {MAX_PROMPT_CHARS} ký tự"));
        }
        let specs: Vec<StudioSessionSpec> = match req.mode.as_str() {
            "single" => {
                let p = req.provider.clone().unwrap_or_else(|| "claude".into());
                vec![StudioSessionSpec {
                    provider: p,
                    model: req.model.clone().unwrap_or_default(),
                    effort: req.effort.clone().unwrap_or_default(),
                    label: Some("Phiên #1".into()),
                }]
            }
            "multi_same_ai" => {
                let n = req.count.unwrap_or(2).clamp(2, MAX_PARALLEL_SESSIONS);
                let p = req.provider.clone().unwrap_or_else(|| "claude".into());
                (1..=n)
                    .map(|i| StudioSessionSpec {
                        provider: p.clone(),
                        model: req.model.clone().unwrap_or_default(),
                        effort: req.effort.clone().unwrap_or_default(),
                        label: Some(format!("Phương án #{i}")),
                    })
                    .collect()
            }
            "multi_ai" | "custom_batch" => {
                if req.sessions.is_empty() {
                    return Err("chọn ít nhất một phiên AI để chạy song song".into());
                }
                if req.sessions.len() > MAX_PARALLEL_SESSIONS {
                    return Err(format!("tối đa {MAX_PARALLEL_SESSIONS} phiên song song mỗi lượt"));
                }
                req.sessions
                    .iter()
                    .enumerate()
                    .map(|(i, s)| StudioSessionSpec {
                        provider: s.provider.trim().to_string(),
                        model: s.model.trim().to_string(),
                        effort: s.effort.trim().to_string(),
                        label: s.label.clone().filter(|l| !l.trim().is_empty()).or_else(|| Some(format!("{} #{}", s.provider.to_uppercase(), i + 1))),
                    })
                    .collect()
            }
            other => return Err(format!("chế độ chạy không hợp lệ: {other}")),
        };

        for s in &specs {
            validate_provider_model_effort(&s.provider, &s.model, &s.effort)?;
        }
        Ok(specs)
    }

    pub async fn launch(
        self: &Arc<Self>,
        title: String,
        skill_id: Option<String>,
        visibility: String,
        prepared: Vec<PreparedSession>,
        on_done: Option<PostSessionHook>,
    ) -> anyhow::Result<StudioRunDetail> {
        anyhow::ensure!(!prepared.is_empty(), "không có phiên nào để chạy");
        let params_json = serde_json::json!({
            "sessionCount": prepared.len(),
            "skillId": skill_id,
            "providers": prepared.iter().map(|p| p.spec.provider.clone()).collect::<Vec<_>>(),
        });
        let job = jobs::create(&self.db, "studio", &title, params_json, 5).await?;
        jobs::mark_running(&self.db, &job.id, Some(format!("{} phiên AI", prepared.len()))).await?;

        let mut session_rows = vec![];
        let mut run_items = vec![];
        for (idx, item) in prepared.into_iter().enumerate() {
            let sid = doto_core::new_id();
            let session_index = idx + 1;
            let (args, stdin_payload, preview) =
                build_invocation(&item.spec.provider, &item.spec.model, &item.spec.effort, &item.full_prompt, item.skill_dir.as_deref(), &item.output_subdir);
            let label = item.spec.label.clone().unwrap_or_else(|| format!("Phiên #{session_index}"));
            let out_sub_str = item.output_subdir.to_string_lossy().into_owned();
            let now = doto_core::now_rfc3339();
            let (sid2, jid2, lbl2, prov2, mod2, eff2, sk2, prev2, out2, now2) = (
                sid.clone(),
                job.id.clone(),
                label.clone(),
                item.spec.provider.clone(),
                item.spec.model.clone(),
                item.spec.effort.clone(),
                skill_id.clone(),
                preview.clone(),
                out_sub_str.clone(),
                now.clone(),
            );
            self.db
                .call(move |c| {
                    c.execute(
                        "INSERT INTO studio_sessions(id,job_id,session_index,label,provider,model,effort,skill_id,status,command_preview,output_subdir,started_at)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'running',?9,?10,?11)",
                        params![sid2, jid2, session_index as i64, lbl2, prov2, mod2, eff2, sk2, prev2, out2, now2],
                    )?;
                    Ok(())
                })
                .await?;
            let token = CancellationToken::new();
            self.cancels.lock().unwrap().insert(sid.clone(), token.clone());
            session_rows.push(StudioSession {
                id: sid.clone(),
                job_id: job.id.clone(),
                session_index,
                label,
                provider: item.spec.provider.clone(),
                model: item.spec.model.clone(),
                effort: item.spec.effort.clone(),
                skill_id: skill_id.clone(),
                status: "running".into(),
                command_preview: preview,
                output_subdir: out_sub_str,
                result_text: String::new(),
                log: vec![],
                error: None,
                exit_code: None,
                output_video_ids: vec![],
                started_at: Some(now),
                finished_at: None,
            });
            run_items.push((sid, item, args, stdin_payload, token));
        }

        let job_now = jobs::get(&self.db, &job.id).await?.unwrap_or(job);
        self.events.emit("job", &job_now);

        let runner = self.clone();
        let jid = job_now.id.clone();
        let total = run_items.len();
        tokio::spawn(async move {
            let mut handles = vec![];
            for (sid, item, args, stdin_payload, token) in run_items {
                let r2 = runner.clone();
                let jid2 = jid.clone();
                let vis2 = visibility.clone();
                let hook = on_done.clone();
                handles.push(tokio::spawn(async move { r2.execute_one(&jid2, &sid, item, args, stdin_payload, vis2, hook, token).await }));
            }
            let mut succeeded = 0usize;
            let mut canceled = 0usize;
            let mut first_err: Option<String> = None;
            let mut first_video: Option<String> = None;
            for (done_idx, h) in handles.into_iter().enumerate() {
                if let Ok((st, err, vids)) = h.await {
                    match st.as_str() {
                        "succeeded" => succeeded += 1,
                        "canceled" => canceled += 1,
                        _ => {
                            if first_err.is_none() {
                                first_err = err;
                            }
                        }
                    }
                    if first_video.is_none() {
                        first_video = vids.into_iter().next();
                    }
                }
                let _ = jobs::update_progress(&runner.db, &jid, (done_idx + 1) as f64 / total as f64, None, None).await;
                if let Ok(Some(j)) = jobs::get(&runner.db, &jid).await {
                    runner.events.emit("job", &j);
                }
            }
            let (final_status, final_err) = if succeeded == total {
                ("succeeded", None)
            } else if canceled == total {
                ("canceled", None)
            } else if succeeded > 0 {
                ("succeeded", Some(format!("hoàn tất {succeeded}/{total} phiên ({})", first_err.unwrap_or_default())))
            } else {
                ("failed", Some(first_err.unwrap_or_else(|| "tất cả các phiên AI đều thất bại".into())))
            };
            let _ = jobs::finish(&runner.db, &jid, final_status, final_err, first_video).await;
            if let Ok(Some(j)) = jobs::get(&runner.db, &jid).await {
                runner.events.emit("job", &j);
            }
        });

        Ok(StudioRunDetail { job: job_now, sessions: session_rows })
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_one(
        &self,
        job_id: &str,
        session_id: &str,
        item: PreparedSession,
        args: Vec<String>,
        stdin_payload: Option<String>,
        visibility: String,
        hook: Option<PostSessionHook>,
        cancel: CancellationToken,
    ) -> (String, Option<String>, Vec<String>) {
        let _ = tokio::fs::create_dir_all(&item.output_subdir).await;
        let _ = tokio::fs::create_dir_all(&item.working_dir).await;
        self.append_session_log(session_id, &format!("$ {}", item.spec.provider)).await;
        let _ = jobs::append_log(&self.db, job_id, &format!("[{}] khởi chạy {}", item.spec.provider, item.output_subdir.display())).await;

        let mut cmd = wrap_cli_command(&item.binary, &args);
        cmd.current_dir(&item.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin_payload.is_some() { Stdio::piped() } else { Stdio::null() })
            .env("NO_COLOR", "1")
            .env("DOTO_OUTPUT_DIR", &item.output_subdir);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("không khởi chạy được {}: {e}", item.binary.display());
                self.finish_session(session_id, "failed", Some(msg.clone()), None, vec![]).await;
                self.cancels.lock().unwrap().remove(session_id);
                return ("failed".into(), Some(msg), vec![]);
            }
        };
        let pid = child.id();

        if let (Some(payload), Some(mut stdin)) = (stdin_payload, child.stdin.take()) {
            tokio::spawn(async move {
                let _ = stdin.write_all(payload.as_bytes()).await;
                let _ = stdin.shutdown().await;
            });
        }

        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");

        let me_out = self.db.clone();
        let ev_out = self.events.clone();
        let sid_out = session_id.to_string();
        let prov = item.spec.provider.clone();
        let stream_err: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let stream_err_cl = stream_err.clone();

        let t_out = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let (log_msg, res_txt, err_txt) = parse_stream_json_line(&prov, &line);
                if let Some(e) = err_txt {
                    *stream_err_cl.lock().unwrap() = Some(e);
                }
                if log_msg.is_some() || res_txt.is_some() {
                    update_session_stream(&me_out, &ev_out, &sid_out, log_msg.as_deref(), res_txt.as_deref()).await;
                }
            }
        });

        let me_err = self.db.clone();
        let ev_err = self.events.clone();
        let sid_err = session_id.to_string();
        let t_err = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let clean = format!("[stderr] {}", doto_core::redact_secrets(trimmed));
                    update_session_stream(&me_err, &ev_err, &sid_err, Some(&clean), None).await;
                }
            }
        });

        let wait_res = tokio::select! {
            _ = cancel.cancelled() => {
                kill_process_tree(&mut child, pid).await;
                Err("CANCELED")
            }
            r = tokio::time::timeout(std::time::Duration::from_secs(45 * 60), child.wait()) => {
                match r {
                    Ok(Ok(status)) => Ok(status),
                    Ok(Err(_)) => Err("WAIT_ERR"),
                    Err(_) => {
                        kill_process_tree(&mut child, pid).await;
                        Err("TIMEOUT")
                    }
                }
            }
        };

        let _ = t_out.await;
        let _ = t_err.await;
        self.cancels.lock().unwrap().remove(session_id);

        let parsed_err = stream_err.lock().unwrap().clone();
        match wait_res {
            Err("CANCELED") => {
                self.finish_session(session_id, "canceled", Some("đã dừng theo yêu cầu".into()), None, vec![]).await;
                let _ = jobs::append_log(&self.db, job_id, &format!("[{}] đã huỷ phiên", item.spec.provider)).await;
                ("canceled".into(), None, vec![])
            }
            Err("TIMEOUT") => {
                let m = "quá thời gian tối đa 45 phút".to_string();
                self.finish_session(session_id, "failed", Some(m.clone()), None, vec![]).await;
                ("failed".into(), Some(m), vec![])
            }
            Err(_) => {
                let m = "lỗi chờ tiến trình con".to_string();
                self.finish_session(session_id, "failed", Some(m.clone()), None, vec![]).await;
                ("failed".into(), Some(m), vec![])
            }
            Ok(status) => {
                let code = status.code();
                if status.success() && parsed_err.is_none() {
                    let vids = match hook {
                        Some(h) => h(job_id.to_string(), item.output_subdir.clone(), visibility).await,
                        None => vec![],
                    };
                    self.finish_session(session_id, "succeeded", None, code, vids.clone()).await;
                    let _ = jobs::append_log(&self.db, job_id, &format!("[{}] hoàn tất ({} video mới)", item.spec.provider, vids.len())).await;
                    ("succeeded".into(), None, vids)
                } else {
                    let m = parsed_err.unwrap_or_else(|| format!("tiến trình kết thúc với mã {:?}", code));
                    self.finish_session(session_id, "failed", Some(m.clone()), code, vec![]).await;
                    let _ = jobs::append_log(&self.db, job_id, &format!("[{}] lỗi: {m}", item.spec.provider)).await;
                    ("failed".into(), Some(m), vec![])
                }
            }
        }
    }

    async fn append_session_log(&self, session_id: &str, line: &str) {
        update_session_stream(&self.db, &self.events, session_id, Some(line), None).await;
    }

    async fn finish_session(&self, session_id: &str, status: &str, error: Option<String>, exit_code: Option<i32>, videos: Vec<String>) {
        let (sid, st, err, vids_json, now) = (
            session_id.to_string(),
            status.to_string(),
            error.map(|e| doto_core::redact_secrets(&e)),
            serde_json::to_string(&videos).unwrap_or_else(|_| "[]".into()),
            doto_core::now_rfc3339(),
        );
        let _ = self
            .db
            .call(move |c| {
                c.execute(
                    "UPDATE studio_sessions SET status=?2, error=?3, exit_code=?4, output_video_ids=?5, finished_at=?6 WHERE id=?1",
                    params![sid, st, err, exit_code, vids_json, now],
                )?;
                Ok(())
            })
            .await;
        if let Ok(Some(s)) = self.get_session(session_id).await {
            self.events.emit("studio", &s);
        }
    }

    pub async fn cancel_session(&self, session_id: &str) -> bool {
        if let Some(tok) = self.cancels.lock().unwrap().get(session_id).cloned() {
            tok.cancel();
            return true;
        }
        false
    }

    pub async fn cancel_job(&self, job_id: &str) -> anyhow::Result<usize> {
        let sessions = self.list_sessions_for_job(job_id).await?;
        let mut n = 0;
        for s in sessions {
            if self.cancel_session(&s.id).await {
                n += 1;
            }
        }
        Ok(n)
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<StudioSession>> {
        let sid = session_id.to_string();
        self.db.call(move |c| Ok(c.query_row(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE id=?1"), [sid], session_from_row).optional()?)).await
    }

    pub async fn list_sessions_for_job(&self, job_id: &str) -> anyhow::Result<Vec<StudioSession>> {
        let jid = job_id.to_string();
        self.db
            .call(move |c| {
                let mut st = c.prepare(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE job_id=?1 ORDER BY session_index ASC"))?;
                let v = st.query_map([jid], session_from_row)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            })
            .await
    }

    pub async fn list_runs(&self, limit: u32) -> anyhow::Result<Vec<StudioRunDetail>> {
        let lim = limit.clamp(1, 100);
        let studio_jobs: Vec<jobs::Job> = self
            .db
            .call(move |c| {
                let mut st = c.prepare(
                    "SELECT id,kind,status,title,progress,eta_seconds,speed,encoder,error,attempts,output_video_id,created_at,started_at,finished_at,priority
                     FROM jobs WHERE kind='studio' ORDER BY created_at DESC LIMIT ?1",
                )?;
                let rows = st
                    .query_map([lim], |r| {
                        Ok(jobs::Job {
                            id: r.get(0)?,
                            kind: r.get(1)?,
                            status: r.get(2)?,
                            title: r.get(3)?,
                            progress: r.get(4)?,
                            eta_seconds: r.get(5)?,
                            speed: r.get(6)?,
                            encoder: r.get(7)?,
                            error: r.get(8)?,
                            attempts: r.get(9)?,
                            output_video_id: r.get(10)?,
                            created_at: r.get(11)?,
                            started_at: r.get(12)?,
                            finished_at: r.get(13)?,
                            priority: r.get(14)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        let mut out = Vec::with_capacity(studio_jobs.len());
        for j in studio_jobs {
            let sessions = self.list_sessions_for_job(&j.id).await?;
            out.push(StudioRunDetail { job: j, sessions });
        }
        Ok(out)
    }

    pub async fn get_run(&self, job_id: &str) -> anyhow::Result<Option<StudioRunDetail>> {
        let Some(j) = jobs::get(&self.db, job_id).await? else { return Ok(None) };
        if j.kind != "studio" {
            return Ok(None);
        }
        let sessions = self.list_sessions_for_job(job_id).await?;
        Ok(Some(StudioRunDetail { job: j, sessions }))
    }
}

const SESSION_COLS: &str =
    "id,job_id,session_index,label,provider,model,effort,skill_id,status,command_preview,output_subdir,result_text,log,error,exit_code,output_video_ids,started_at,finished_at";

fn session_from_row(r: &rusqlite::Row) -> rusqlite::Result<StudioSession> {
    let raw_log: String = r.get(12)?;
    let raw_vids: String = r.get(15)?;
    Ok(StudioSession {
        id: r.get(0)?,
        job_id: r.get(1)?,
        session_index: r.get::<_, i64>(2)? as usize,
        label: r.get(3)?,
        provider: r.get(4)?,
        model: r.get(5)?,
        effort: r.get(6)?,
        skill_id: r.get(7)?,
        status: r.get(8)?,
        command_preview: r.get(9)?,
        output_subdir: r.get(10)?,
        result_text: r.get(11)?,
        log: raw_log.lines().map(String::from).collect(),
        error: r.get(13)?,
        exit_code: r.get(14)?,
        output_video_ids: serde_json::from_str(&raw_vids).unwrap_or_default(),
        started_at: r.get(16)?,
        finished_at: r.get(17)?,
    })
}

async fn update_session_stream(db: &Db, events: &EventBus, session_id: &str, log_line: Option<&str>, result_text: Option<&str>) {
    let sid = session_id.to_string();
    let add_line = log_line.map(|l| doto_core::redact_secrets(l.trim_end()));
    let new_res = result_text.map(doto_core::redact_secrets);
    let sid2 = sid.clone();
    let updated = db
        .call(move |c| {
            if let Some(line) = add_line {
                let mut cur: String = c.query_row("SELECT log FROM studio_sessions WHERE id=?1", [&sid2], |r| r.get(0)).unwrap_or_default();
                cur.push_str(&line);
                cur.push('\n');
                if cur.len() > MAX_SESSION_LOG_BYTES {
                    let cut = cur.len() - MAX_SESSION_LOG_BYTES;
                    let idx = cur[cut..].find('\n').map(|i| cut + i + 1).unwrap_or(cut);
                    cur = format!("…(đã cắt bớt)\n{}", &cur[idx..]);
                }
                c.execute("UPDATE studio_sessions SET log=?2 WHERE id=?1", params![sid2, cur])?;
            }
            if let Some(res) = new_res {
                c.execute("UPDATE studio_sessions SET result_text=?2 WHERE id=?1", params![sid2, res])?;
            }
            Ok(c.query_row(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE id=?1"), [&sid2], session_from_row).optional()?)
        })
        .await;
    if let Ok(Some(s)) = updated {
        events.emit("studio", &s);
    }
}

fn wrap_cli_command(binary: &Path, args: &[String]) -> tokio::process::Command {
    let mut cmd = command(binary);
    cmd.args(args);
    cmd
}

async fn kill_process_tree(child: &mut tokio::process::Child, pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(p) = pid {
        let _ = command(Path::new("taskkill")).args(["/PID", &p.to_string(), "/T", "/F"]).output().await;
    }
    #[cfg(not(windows))]
    let _ = pid;
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_all_four_modes_and_validates() {
        let mut req = StudioRunRequest {
            mode: "single".into(),
            prompt: "Dựng video bản tin 60s".into(),
            title: None,
            skill_id: None,
            provider: Some("claude".into()),
            model: Some("claude-sonnet-4-6".into()),
            effort: Some("high".into()),
            count: None,
            sessions: vec![],
            upload_ids: vec![],
            include_memory_hint: false,
            visibility: None,
            confirm: false,
        };
        assert!(StudioRunner::expand_specs(&req).is_err(), "thiếu confirm phải từ chối");
        req.confirm = true;
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 1);

        req.mode = "multi_same_ai".into();
        req.count = Some(3);
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 3);

        req.mode = "multi_ai".into();
        req.sessions = vec![
            StudioSessionSpec { provider: "antigravity".into(), model: "gemini-2.5-pro".into(), effort: "high".into(), label: None },
            StudioSessionSpec { provider: "claude".into(), model: "claude-opus-4-6".into(), effort: "high".into(), label: None },
            StudioSessionSpec { provider: "codex".into(), model: "gpt-5.4".into(), effort: "medium".into(), label: None },
        ];
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 3);

        req.sessions[0].model = "bad;rm -rf".into();
        assert!(StudioRunner::expand_specs(&req).is_err());
    }

    #[test]
    fn parses_stream_json_and_redacts_tokens() {
        let claude_line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Xin chào ghp_secretToken1234567890"}]}}"#;
        let (log, res, err) = parse_stream_json_line("claude", claude_line);
        assert!(err.is_none());
        assert!(!log.unwrap().contains("ghp_secret"));
        assert!(res.unwrap().contains("[REDACTED]"));

        let codex_line = r#"{"type":"item.completed","item":{"type":"agent_message","text":"Đã tạo xong video"}}"#;
        let (_, res2, _) = parse_stream_json_line("codex", codex_line);
        assert_eq!(res2.as_deref(), Some("Đã tạo xong video"));

        let agy_line = r#"{"event":"result","result":{"status":"SUCCESS","response":"Hoàn thành bản nháp"}}"#;
        let (_, res3, _) = parse_stream_json_line("antigravity", agy_line);
        assert_eq!(res3.as_deref(), Some("Hoàn thành bản nháp"));
    }

    #[tokio::test]
    async fn executes_parallel_sessions_with_isolated_outputs() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let events = EventBus::default();
        let tools = ToolManager::new(db.clone(), events.clone(), tmp.path());
        let runner = StudioRunner::new(db.clone(), events, tools);

        #[cfg(windows)]
        let fake_bin = {
            let p = tmp.path().join("fake-ai.cmd");
            std::fs::write(&p, "@echo off\r\necho {\"type\":\"result\",\"result\":\"OK from fake AI\"}\r\n").unwrap();
            p
        };
        #[cfg(unix)]
        let fake_bin = {
            use std::os::unix::fs::PermissionsExt;
            let p = tmp.path().join("fake-ai.sh");
            std::fs::write(&p, "#!/bin/sh\necho '{\"type\":\"result\",\"result\":\"OK from fake AI\"}'\n").unwrap();
            let mut perms = std::fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&p, perms).unwrap();
            p
        };
        runner.set_binary_override("claude", fake_bin.clone());
        runner.set_binary_override("codex", fake_bin);

        let prepared = vec![
            PreparedSession {
                spec: StudioSessionSpec { provider: "claude".into(), model: "".into(), effort: "high".into(), label: Some("S1".into()) },
                binary: runner.locate_provider("claude").unwrap(),
                working_dir: tmp.path().join("w1"),
                skill_dir: None,
                output_subdir: tmp.path().join("out-s1"),
                full_prompt: "Test 1".into(),
            },
            PreparedSession {
                spec: StudioSessionSpec { provider: "codex".into(), model: "".into(), effort: "medium".into(), label: Some("S2".into()) },
                binary: runner.locate_provider("codex").unwrap(),
                working_dir: tmp.path().join("w2"),
                skill_dir: None,
                output_subdir: tmp.path().join("out-s2"),
                full_prompt: "Test 2".into(),
            },
        ];

        let run = runner.launch("Chạy song song 2 AI".into(), None, "private".into(), prepared, None).await.unwrap();
        assert_eq!(run.sessions.len(), 2);

        for _ in 0..40 {
            let cur = runner.get_run(&run.job.id).await.unwrap().unwrap();
            if cur.job.status == "succeeded" {
                assert_eq!(cur.sessions[0].status, "succeeded");
                assert_eq!(cur.sessions[1].status, "succeeded");
                assert!(tmp.path().join("out-s1").is_dir());
                assert!(tmp.path().join("out-s2").is_dir());
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("timeout chờ studio run hoàn tất");
    }
}

//! doto-media: FFmpeg orchestrator + hàng đợi render bị giới hạn bởi ResourceManager.

pub mod encoders;
pub mod ffmpeg;
pub mod library;
pub mod probe;
pub mod render;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use doto_core::{events::EventBus, jobs, paths::AppPaths, settings, Db};
use doto_system::ResourceManager;

pub use encoders::{EncoderBenchmark, EncoderInfo, EncoderProbe};
pub use library::VideoAsset;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegStatus {
    pub found: bool,
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub version: Option<String>,
    pub license_flags: Vec<String>,
    pub encoders: Vec<EncoderInfo>,
    pub encoders_probed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderSegmentReq {
    pub video_id: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub title: String,
    pub segments: Vec<RenderSegmentReq>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub crf: Option<i64>,
    pub preset: Option<String>,
    pub audio_bitrate_kbps: Option<i64>,
    pub encoder: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    InsufficientDisk(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub scanned: usize,
    pub added_or_updated: usize,
    pub unchanged: usize,
    pub missing: usize,
    pub errors: Vec<String>,
}

struct RunningJob {
    cancel: CancellationToken,
    gpu: bool,
}

pub struct MediaService {
    db: Db,
    paths: AppPaths,
    rm: ResourceManager,
    events: EventBus,
    ffmpeg: RwLock<Option<ffmpeg::FfmpegInfo>>,
    ffmpeg_error: RwLock<Option<String>>,
    encoders: RwLock<EncoderProbe>,
    running: Mutex<HashMap<String, RunningJob>>,
    wake: Notify,
    throttle: RwLock<Option<String>>,
}

impl MediaService {
    pub fn new(db: Db, paths: AppPaths, rm: ResourceManager, events: EventBus) -> Arc<Self> {
        Arc::new(Self {
            db,
            paths,
            rm,
            events,
            ffmpeg: RwLock::new(None),
            ffmpeg_error: RwLock::new(None),
            encoders: RwLock::new(EncoderProbe::default()),
            running: Mutex::new(HashMap::new()),
            wake: Notify::new(),
            throttle: RwLock::new(None),
        })
    }

    /// Dò FFmpeg theo cấu hình hiện tại. Nạp kết quả probe encoder đã lưu (nếu cùng phiên bản).
    pub async fn refresh_ffmpeg(&self) -> FfmpegStatus {
        let app: settings::AppSettings = settings::get(&self.db, settings::KEY_APP).await.unwrap_or_default();
        match ffmpeg::discover(app.ffmpeg_path.as_deref(), app.ffprobe_path.as_deref()).await {
            Ok(info) => {
                let stored: EncoderProbe = settings::get(&self.db, settings::KEY_ENCODERS).await.unwrap_or_default();
                if stored.ffmpeg_version.as_deref() == Some(info.version.as_str()) {
                    *self.encoders.write().unwrap() = stored;
                }
                *self.ffmpeg.write().unwrap() = Some(info);
                *self.ffmpeg_error.write().unwrap() = None;
            }
            Err(e) => {
                *self.ffmpeg.write().unwrap() = None;
                *self.ffmpeg_error.write().unwrap() = Some(e.to_string());
            }
        }
        self.wake.notify_one();
        self.status()
    }

    pub async fn probe_encoders(&self) -> Result<FfmpegStatus, MediaError> {
        let info = self.ffmpeg_info()?;
        let probe = encoders::probe_all(&info.ffmpeg_path, Some(info.version.clone())).await?;
        settings::set(&self.db, settings::KEY_ENCODERS, &probe).await?;
        *self.encoders.write().unwrap() = probe;
        Ok(self.status())
    }

    pub async fn benchmark_encoders(&self) -> Result<Vec<EncoderBenchmark>, MediaError> {
        let info = self.ffmpeg_info()?;
        let mut probe = self.encoders.read().unwrap().clone();
        if probe.encoders.is_empty() {
            probe = encoders::probe_all(&info.ffmpeg_path, Some(info.version.clone())).await?;
            settings::set(&self.db, settings::KEY_ENCODERS, &probe).await?;
            *self.encoders.write().unwrap() = probe.clone();
        }
        let threads = self.rm.limits().ffmpeg_threads;
        Ok(encoders::benchmark_working(&info.ffmpeg_path, &probe, threads).await)
    }

    pub fn status(&self) -> FfmpegStatus {
        let f = self.ffmpeg.read().unwrap().clone();
        let e = self.encoders.read().unwrap().clone();
        FfmpegStatus {
            found: f.is_some(),
            ffmpeg_path: f.as_ref().map(|i| i.ffmpeg_path.to_string_lossy().into_owned()),
            ffprobe_path: f.as_ref().map(|i| i.ffprobe_path.to_string_lossy().into_owned()),
            version: f.as_ref().map(|i| i.version.clone()),
            license_flags: f.map(|i| i.license_flags).unwrap_or_default(),
            encoders: e.encoders,
            encoders_probed_at: e.probed_at,
            error: self.ffmpeg_error.read().unwrap().clone(),
        }
    }

    fn ffmpeg_info(&self) -> Result<ffmpeg::FfmpegInfo, MediaError> {
        self.ffmpeg.read().unwrap().clone().ok_or_else(|| MediaError::Unavailable("chưa cấu hình FFmpeg — vào Cài đặt › FFmpeg để chỉ định đường dẫn".into()))
    }

    pub fn running_counts(&self) -> (usize, usize) {
        let r = self.running.lock().unwrap();
        (r.len(), r.values().filter(|j| j.gpu).count())
    }

    pub fn throttle_reason(&self) -> Option<String> {
        self.throttle.read().unwrap().clone()
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }

    async fn output_dir(&self) -> PathBuf {
        let app: settings::AppSettings = settings::get(&self.db, settings::KEY_APP).await.unwrap_or_default();
        app.output_dir.map(PathBuf::from).unwrap_or_else(|| self.paths.default_exports_dir())
    }

    // ---------------- Thư viện ----------------

    /// Quét các thư mục nguồn đã cấp quyền: ffprobe file mới/đổi, tạo thumbnail, đánh dấu file mất.
    pub async fn scan_sources(&self) -> Result<ScanSummary, MediaError> {
        let info = self.ffmpeg_info()?;
        let app: settings::AppSettings = settings::get(&self.db, settings::KEY_APP).await?;
        let known = library::known_files(&self.db).await?;
        let mut sum = ScanSummary::default();
        let mut seen = vec![];
        for dir in &app.video_source_dirs {
            let root = PathBuf::from(dir);
            if !root.is_dir() {
                sum.errors.push(format!("thư mục không tồn tại: {dir}"));
                continue;
            }
            let files = tokio::task::spawn_blocking(move || library::discover_files(&root, 6)).await.map_err(anyhow::Error::from)?;
            for f in files {
                sum.scanned += 1;
                let key = f.to_string_lossy().into_owned();
                seen.push(key.clone());
                let meta = match tokio::fs::metadata(&f).await {
                    Ok(m) => m,
                    Err(e) => {
                        sum.errors.push(format!("{}: {e}", f.display()));
                        continue;
                    }
                };
                let mtime = meta.modified().ok().map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
                if known.get(&key).map(|(s, m)| *s == meta.len() as i64 && *m == mtime).unwrap_or(false) {
                    sum.unchanged += 1;
                    continue;
                }
                match self.index_file(&info, &f, "source", meta.len() as i64, mtime, "private", None, None).await {
                    Ok(_) => sum.added_or_updated += 1,
                    Err(e) => sum.errors.push(format!("{}: {e}", f.file_name().unwrap_or_default().to_string_lossy())),
                }
            }
        }
        sum.missing = library::mark_missing(&self.db, seen).await?;
        Ok(sum)
    }

    #[allow(clippy::too_many_arguments)]
    async fn index_file(
        &self,
        info: &ffmpeg::FfmpegInfo,
        f: &Path,
        kind: &str,
        size: i64,
        mtime: Option<String>,
        visibility: &str,
        title: Option<String>,
        job_id: Option<String>,
    ) -> anyhow::Result<VideoAsset> {
        let mi = probe::probe(&info.ffprobe_path, f).await?;
        let title = title.unwrap_or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let id = library::upsert(&self.db, library::NewAsset { kind, title, path: f, size, mtime, info: &mi, visibility, job_id }).await?;
        let thumb = self.paths.thumbnails_dir().join(format!("{id}.jpg"));
        if probe::thumbnail(&info.ffmpeg_path, f, mi.duration_ms, &thumb).await.is_ok() {
            library::set_thumbnail(&self.db, &id, &thumb).await?;
        }
        let asset = library::get(&self.db, &id).await?.ok_or_else(|| anyhow::anyhow!("asset biến mất"))?;
        self.events.emit_scoped("video", &asset, asset.kind == "export" && asset.visibility == "viewers");
        Ok(asset)
    }

    // ---------------- Render ----------------

    async fn build_spec(&self, req: &RenderRequest) -> Result<(render::RenderSpec, String), MediaError> {
        let title = req.title.trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(MediaError::Validation("tiêu đề phải từ 1–200 ký tự".into()));
        }
        if req.segments.is_empty() {
            return Err(MediaError::Validation("cần ít nhất một đoạn nguồn".into()));
        }
        let mut segs = vec![];
        let mut first: Option<VideoAsset> = None;
        for s in &req.segments {
            let a = library::get(&self.db, &s.video_id).await?.ok_or_else(|| MediaError::NotFound(format!("không tìm thấy video {}", s.video_id)))?;
            if a.status != "ready" || !a.file_path.is_file() {
                return Err(MediaError::Validation(format!("video \"{}\" không còn trên đĩa", a.title)));
            }
            let dur = a.duration_ms.unwrap_or(0);
            let start = s.start_ms.unwrap_or(0).max(0);
            let end = s.end_ms.unwrap_or(dur).min(if dur > 0 { dur } else { i64::MAX });
            if end <= start {
                return Err(MediaError::Validation(format!("đoạn của \"{}\": thời điểm kết thúc phải sau thời điểm bắt đầu", a.title)));
            }
            segs.push(render::SegmentInput { path: a.file_path.clone(), start_ms: start, duration_ms: end - start, has_audio: a.has_audio });
            first.get_or_insert(a);
        }
        let first = first.expect("ít nhất 1 đoạn");
        let even = |v: i64| (v / 2 * 2).max(16);
        let width = even(req.width.or(first.width).unwrap_or(1280));
        let height = even(req.height.or(first.height).unwrap_or(720));
        let encoder = self.pick_encoder(req.encoder.as_deref())?;
        let spec = render::RenderSpec {
            segments: segs,
            width,
            height,
            fps: req.fps.unwrap_or(30.0),
            crf: req.crf.unwrap_or(23),
            preset: req.preset.clone().unwrap_or_else(|| "veryfast".into()),
            audio_bitrate_kbps: req.audio_bitrate_kbps.unwrap_or(160),
            encoder,
            threads: self.rm.limits().ffmpeg_threads,
        };
        spec.validate().map_err(MediaError::Validation)?;
        Ok((spec, title.to_string()))
    }

    fn pick_encoder(&self, wanted: Option<&str>) -> Result<String, MediaError> {
        let probe = self.encoders.read().unwrap();
        match wanted.unwrap_or("auto") {
            "auto" => Ok(probe.best_hardware().filter(|_| self.rm.limits().max_gpu_jobs > 0).unwrap_or("libx264").to_string()),
            "software" | "libx264" => Ok("libx264".into()),
            other if probe.is_working(other) => Ok(other.to_string()),
            other => Err(MediaError::Validation(format!("encoder {other} chưa được kiểm chứng trên máy này — chạy lại kiểm tra encoder"))),
        }
    }

    /// Ước lượng dung lượng output và kiểm tra đĩa trống trước khi xếp hàng.
    fn disk_preflight(dir: &Path, spec: &render::RenderSpec) -> Result<(), MediaError> {
        let secs = spec.total_duration_ms() as f64 / 1000.0;
        let video_bps = (spec.width * spec.height) as f64 * spec.fps * 0.12;
        let est = (secs * (video_bps + spec.audio_bitrate_kbps as f64 * 1000.0) / 8.0) as u64;
        let need = est.saturating_mul(3) / 2 + 200 * 1024 * 1024;
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let canon = doto_core::paths::dunce_canonical(dir).unwrap_or_else(|| dir.to_path_buf());
        let best = disks.list().iter().filter(|d| canon.starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len());
        if let Some(d) = best {
            if d.available_space() < need {
                return Err(MediaError::InsufficientDisk(format!(
                    "không đủ dung lượng: cần ~{} MB, còn {} MB",
                    need / 1024 / 1024,
                    d.available_space() / 1024 / 1024
                )));
            }
        }
        Ok(())
    }

    pub async fn submit_render(&self, req: RenderRequest) -> Result<jobs::Job, MediaError> {
        self.ffmpeg_info()?;
        let (spec, title) = self.build_spec(&req).await?;
        let out_dir = self.output_dir().await;
        tokio::fs::create_dir_all(&out_dir).await.map_err(|e| MediaError::Validation(format!("không tạo được thư mục output: {e}")))?;
        Self::disk_preflight(&out_dir, &spec)?;
        let visibility = match req.visibility.as_deref() {
            Some("viewers") => "viewers",
            _ => "private",
        };
        let params = serde_json::json!({ "request": req, "spec": spec, "visibility": visibility });
        let job = jobs::create(&self.db, "render", &title, params, 0).await?;
        self.events.emit("job", &job);
        self.wake.notify_one();
        Ok(job)
    }

    pub async fn cancel(&self, job_id: &str) -> Result<bool, MediaError> {
        if let Some(r) = self.running.lock().unwrap().get(job_id) {
            r.cancel.cancel();
            return Ok(true);
        }
        let j = jobs::get(&self.db, job_id).await?.ok_or_else(|| MediaError::NotFound("không tìm thấy job".into()))?;
        if j.status == "queued" {
            jobs::finish(&self.db, job_id, "canceled", None, None).await?;
            if let Some(j) = jobs::get(&self.db, job_id).await? {
                self.events.emit("job", &j);
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn retry(&self, job_id: &str) -> Result<bool, MediaError> {
        let ok = jobs::requeue(&self.db, job_id).await?;
        if ok {
            if let Some(j) = jobs::get(&self.db, job_id).await? {
                self.events.emit("job", &j);
            }
            self.wake.notify_one();
        }
        Ok(ok)
    }

    /// Vòng lặp Scheduler: lấy job theo priority/FIFO, chỉ khởi chạy khi ResourceManager cho phép.
    pub fn spawn_scheduler(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let me = self.clone();
        tokio::spawn(async move {
            match jobs::recover_interrupted(&me.db).await {
                Ok(n) if n > 0 => tracing::warn!(n, "đã khôi phục job bị gián đoạn"),
                Err(e) => tracing::error!(error = %e, "không khôi phục được job"),
                _ => {}
            }
            loop {
                if let Err(e) = me.schedule_once().await {
                    tracing::error!(error = %e, "scheduler lỗi");
                }
                tokio::select! {
                    _ = me.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(1000)) => {}
                }
            }
        })
    }

    async fn schedule_once(self: &Arc<Self>) -> anyhow::Result<()> {
        let queued = jobs::queued(&self.db, "render").await?;
        if queued.is_empty() {
            *self.throttle.write().unwrap() = None;
            return Ok(());
        }
        if self.ffmpeg.read().unwrap().is_none() {
            *self.throttle.write().unwrap() = Some("chưa có FFmpeg".into());
            return Ok(());
        }
        for job in queued {
            let params = jobs::params(&self.db, &job.id).await?;
            let spec: render::RenderSpec = match serde_json::from_value(params["spec"].clone()) {
                Ok(s) => s,
                Err(e) => {
                    jobs::finish(&self.db, &job.id, "failed", Some(format!("tham số job hỏng: {e}")), None).await?;
                    continue;
                }
            };
            let wants_gpu = spec.encoder != "libx264";
            let (running, gpu) = self.running_counts();
            if let Err(reason) = self.rm.admit(running, gpu, wants_gpu) {
                *self.throttle.write().unwrap() = Some(reason);
                return Ok(());
            }
            *self.throttle.write().unwrap() = None;
            let token = CancellationToken::new();
            self.running.lock().unwrap().insert(job.id.clone(), RunningJob { cancel: token.clone(), gpu: wants_gpu });
            jobs::mark_running(&self.db, &job.id, Some(spec.encoder.clone())).await?;
            let me = self.clone();
            let vis = params["visibility"].as_str().unwrap_or("private").to_string();
            tokio::spawn(async move {
                let id = job.id.clone();
                let res = me.execute(&job, spec, &vis, token).await;
                me.running.lock().unwrap().remove(&id);
                if let Err(e) = res {
                    let _ = jobs::finish(&me.db, &id, "failed", Some(e.to_string()), None).await;
                }
                if let Ok(Some(j)) = jobs::get(&me.db, &id).await {
                    me.events.emit("job", &j);
                }
                me.wake.notify_one();
            });
        }
        Ok(())
    }

    async fn execute(&self, job: &jobs::Job, mut spec: render::RenderSpec, visibility: &str, cancel: CancellationToken) -> anyhow::Result<()> {
        let info = self.ffmpeg_info().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Thread FFmpeg theo profile hiện tại (profile có thể đã đổi khi job còn trong hàng đợi)
        spec.threads = self.rm.limits().ffmpeg_threads;
        let out_dir = self.output_dir().await;
        tokio::fs::create_dir_all(&out_dir).await?;
        let stem = library::sanitize_file_stem(&job.title);
        let final_path = out_dir.join(format!("{stem}-{}.mp4", &job.id[..8]));
        if let Some(j) = jobs::get(&self.db, &job.id).await? {
            self.events.emit("job", &j);
        }
        let mut outcome = self.run_ffmpeg(&info, &job.id, &spec, &final_path, cancel.clone()).await;
        if let render::RenderOutcome::Failed(err) = &outcome {
            if spec.encoder != "libx264" && !cancel.is_cancelled() {
                jobs::append_log(&self.db, &job.id, &format!("[fallback] encoder {} lỗi ({err}) → chuyển sang libx264", spec.encoder)).await?;
                spec.encoder = "libx264".into();
                jobs::set_encoder(&self.db, &job.id, "libx264").await?;
                outcome = self.run_ffmpeg(&info, &job.id, &spec, &final_path, cancel.clone()).await;
            }
        }
        match outcome {
            render::RenderOutcome::Done => {
                let meta = tokio::fs::metadata(&final_path).await?;
                let asset =
                    self.index_file(&info, &final_path, "export", meta.len() as i64, None, visibility, Some(job.title.clone()), Some(job.id.clone())).await?;
                let preset = serde_json::to_string(&spec)?;
                let (jid, vid) = (job.id.clone(), asset.id.clone());
                self.db
                    .call(move |c| {
                        c.execute(
                            "INSERT INTO exports(id,job_id,video_id,preset,created_at) VALUES(?1,?2,?3,?4,?5)",
                            rusqlite::params![doto_core::new_id(), jid, vid, preset, doto_core::now_rfc3339()],
                        )?;
                        Ok(())
                    })
                    .await?;
                jobs::finish(&self.db, &job.id, "succeeded", None, Some(asset.id)).await?;
            }
            render::RenderOutcome::Canceled => {
                jobs::append_log(&self.db, &job.id, "[hủy] người dùng đã hủy job").await?;
                jobs::finish(&self.db, &job.id, "canceled", None, None).await?;
            }
            render::RenderOutcome::Failed(e) => {
                jobs::finish(&self.db, &job.id, "failed", Some(e), None).await?;
            }
        }
        Ok(())
    }

    async fn run_ffmpeg(
        &self,
        info: &ffmpeg::FfmpegInfo,
        job_id: &str,
        spec: &render::RenderSpec,
        final_path: &Path,
        cancel: CancellationToken,
    ) -> render::RenderOutcome {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let db = self.db.clone();
        let events = self.events.clone();
        let id = job_id.to_string();
        let pump = tokio::spawn(async move {
            let mut last = Instant::now() - Duration::from_secs(5);
            while let Some(u) = rx.recv().await {
                match u {
                    render::RenderUpdate::Progress { ratio, eta_seconds, speed } => {
                        if last.elapsed() >= Duration::from_millis(400) || ratio >= 1.0 {
                            last = Instant::now();
                            let _ = jobs::update_progress(&db, &id, ratio, eta_seconds, speed).await;
                            if let Ok(Some(j)) = jobs::get(&db, &id).await {
                                events.emit("job", &j);
                            }
                        }
                    }
                    render::RenderUpdate::Log(l) => {
                        let _ = jobs::append_log(&db, &id, &l).await;
                    }
                }
            }
        });
        let out = render::run(&info.ffmpeg_path, spec, final_path, tx, cancel).await;
        let _ = pump.await;
        out
    }
}

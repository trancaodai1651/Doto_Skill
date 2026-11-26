#![allow(clippy::field_reassign_with_default)]
//! Integration test với FFmpeg thật. Bỏ qua (không fail) nếu máy không có ffmpeg/ffprobe,
//! trừ khi đặt `DOTO_REQUIRE_FFMPEG=1` (CI bật biến này để bắt buộc chạy).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use doto_core::{events::EventBus, jobs, paths::AppPaths, settings, Db};
use doto_media::{library, MediaService, RenderRequest, RenderSegmentReq};
use doto_system::{HostCapacity, ResourceManager, StoredResourceSettings};

fn ffmpeg_available() -> bool {
    let ok = which::which("ffmpeg").is_ok() && which::which("ffprobe").is_ok();
    if !ok && std::env::var("DOTO_REQUIRE_FFMPEG").as_deref() == Ok("1") {
        panic!("DOTO_REQUIRE_FFMPEG=1 nhưng không tìm thấy ffmpeg/ffprobe");
    }
    ok
}

/// Tạo video mẫu hợp pháp bằng nguồn tổng hợp lavfi (testsrc2 + sine).
fn make_sample(path: &Path, seconds: u32, with_audio: bool, size: &str) {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!("testsrc2=size={size}:rate=25:duration={seconds}"),
    ];
    if with_audio {
        args.extend(["-f".into(), "lavfi".into(), "-i".into(), format!("sine=frequency=440:duration={seconds}")]);
    }
    args.extend(["-c:v".into(), "libx264".into(), "-preset".into(), "ultrafast".into(), "-pix_fmt".into(), "yuv420p".into()]);
    if with_audio {
        args.extend(["-c:a".into(), "aac".into(), "-shortest".into()]);
    }
    args.push(path.to_string_lossy().into_owned());
    let st = std::process::Command::new("ffmpeg").args(&args).status().expect("chạy ffmpeg");
    assert!(st.success(), "tạo video mẫu thất bại");
}

async fn setup(src_dir: &Path, data: &Path) -> (Arc<MediaService>, Db) {
    let paths = AppPaths::resolve(Some(data)).unwrap();
    let db = Db::open(&paths.db_path()).unwrap();
    let mut app = settings::AppSettings::default();
    app.video_source_dirs = vec![src_dir.to_string_lossy().into_owned()];
    settings::set(&db, settings::KEY_APP, &app).await.unwrap();
    let rm = ResourceManager::new(HostCapacity { logical_cores: 4, total_memory_mb: 8192 }, StoredResourceSettings::default());
    let svc = MediaService::new(db.clone(), paths, rm, EventBus::default());
    let st = svc.refresh_ffmpeg().await;
    assert!(st.found, "ffmpeg phải được tìm thấy: {:?}", st.error);
    svc.spawn_scheduler();
    (svc, db)
}

async fn wait_terminal(db: &Db, id: &str, secs: u64) -> jobs::Job {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let j = jobs::get(db, id).await.unwrap().unwrap();
        if matches!(j.status.as_str(), "succeeded" | "failed" | "canceled") {
            return j;
        }
        assert!(std::time::Instant::now() < deadline, "job quá hạn, trạng thái {}", j.status);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scan_trim_concat_render_produces_playable_mp4() {
    if !ffmpeg_available() {
        eprintln!("bỏ qua: không có ffmpeg");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    make_sample(&src.join("clip_a.mp4"), 4, true, "320x240");
    make_sample(&src.join("clip_b.mp4"), 3, false, "640x360"); // không có audio, khác độ phân giải
    let (svc, db) = setup(&src, &tmp.path().join("data")).await;

    let sum = svc.scan_sources().await.unwrap();
    assert_eq!(sum.added_or_updated, 2, "{sum:?}");
    // quét lần 2: không đổi gì
    assert_eq!(svc.scan_sources().await.unwrap().unchanged, 2);

    let vids = library::list(&db, library::ListFilter { kind: Some("source".into()), ..Default::default() }).await.unwrap();
    let a = vids.iter().find(|v| v.file_name == "clip_a.mp4").unwrap();
    let b = vids.iter().find(|v| v.file_name == "clip_b.mp4").unwrap();
    assert!(a.has_thumbnail && a.duration_ms.unwrap() >= 3900);

    let job = svc
        .submit_render(RenderRequest {
            title: "Bản dựng thử".into(),
            segments: vec![
                RenderSegmentReq { video_id: a.id.clone(), start_ms: Some(1000), end_ms: Some(3000) },
                RenderSegmentReq { video_id: b.id.clone(), start_ms: None, end_ms: Some(1500) },
            ],
            width: Some(640),
            height: Some(360),
            encoder: Some("software".into()),
            visibility: Some("viewers".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    let done = wait_terminal(&db, &job.id, 120).await;
    let detail = jobs::detail(&db, &job.id).await.unwrap().unwrap();
    assert_eq!(done.status, "succeeded", "log: {}", detail.log.join("\n"));
    assert_eq!(done.progress, 1.0);
    let out = library::get(&db, done.output_video_id.as_ref().unwrap()).await.unwrap().unwrap();
    assert_eq!(out.kind, "export");
    assert_eq!(out.visibility, "viewers");
    assert_eq!((out.width, out.height), (Some(640), Some(360)));
    assert_eq!(out.video_codec.as_deref(), Some("h264"));
    assert_eq!(out.audio_codec.as_deref(), Some("aac"));
    let d = out.duration_ms.unwrap();
    assert!((3300..=3800).contains(&d), "thời lượng {d}ms (mong đợi ~3500ms)");
    // không còn file .part
    let leftovers: Vec<PathBuf> = std::fs::read_dir(out.file_path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().contains(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_running_job_removes_partial_output() {
    if !ffmpeg_available() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    make_sample(&src.join("long.mp4"), 20, true, "1280x720");
    let (svc, db) = setup(&src, &tmp.path().join("data")).await;
    svc.scan_sources().await.unwrap();
    let v = library::list(&db, Default::default()).await.unwrap().remove(0);
    let job = svc
        .submit_render(RenderRequest {
            title: "Huỷ thử".into(),
            segments: vec![RenderSegmentReq { video_id: v.id, start_ms: None, end_ms: None }],
            width: Some(1920),
            height: Some(1080),
            preset: Some("slow".into()),
            encoder: Some("software".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    // chờ chạy rồi hủy
    for _ in 0..100 {
        if jobs::get(&db, &job.id).await.unwrap().unwrap().status == "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(svc.cancel(&job.id).await.unwrap());
    let j = wait_terminal(&db, &job.id, 30).await;
    assert_eq!(j.status, "canceled");
    assert!(j.output_video_id.is_none());
    let exports = tmp.path().join("data").join("exports");
    let files: Vec<_> = std::fs::read_dir(exports).unwrap().filter_map(|e| e.ok()).collect();
    assert!(files.is_empty(), "không được để lại file dở dang");
    // retry đưa lại hàng đợi
    assert!(svc.retry(&job.id).await.unwrap());
    svc.cancel(&job.id).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_requests_are_rejected() {
    if !ffmpeg_available() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("broken.mp4"), b"this is not a video").unwrap();
    make_sample(&src.join("ok.mp4"), 2, true, "320x240");
    let (svc, db) = setup(&src, &tmp.path().join("data")).await;
    let sum = svc.scan_sources().await.unwrap();
    assert_eq!(sum.added_or_updated, 1);
    assert_eq!(sum.errors.len(), 1, "file hỏng phải báo lỗi: {sum:?}");
    let v = library::list(&db, Default::default()).await.unwrap().remove(0);
    let bad_range = svc
        .submit_render(RenderRequest {
            title: "x".into(),
            segments: vec![RenderSegmentReq { video_id: v.id.clone(), start_ms: Some(1500), end_ms: Some(1000) }],
            ..Default::default()
        })
        .await;
    assert!(bad_range.is_err());
    let unknown = svc
        .submit_render(RenderRequest {
            title: "x".into(),
            segments: vec![RenderSegmentReq { video_id: "nope".into(), ..Default::default() }],
            ..Default::default()
        })
        .await;
    assert!(matches!(unknown, Err(doto_media::MediaError::NotFound(_))));
    let bad_enc = svc
        .submit_render(RenderRequest {
            title: "x".into(),
            segments: vec![RenderSegmentReq { video_id: v.id, ..Default::default() }],
            encoder: Some("h264_fake".into()),
            ..Default::default()
        })
        .await;
    assert!(bad_enc.is_err());
}

//! Integration test API qua HTTP thật: auth, CSRF, RBAC, listener isolation, pairing/revoke,
//! rate limit, Range/416/download, IDOR, path traversal, xử lý token. Render dùng FFmpeg thật (nếu có).

use std::sync::Arc;
use std::time::Duration;

use reqwest::{header, Method, StatusCode};
use serde_json::{json, Value};

use doto_server::{AppState, Options};

struct Client {
    base: String,
    http: reqwest::Client,
    cookie: Option<String>,
    csrf: Option<String>,
}

impl Client {
    fn new(base: &str) -> Self {
        Self { base: base.into(), http: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap(), cookie: None, csrf: None }
    }

    async fn req(&mut self, m: Method, path: &str, body: Option<Value>) -> (StatusCode, Value, reqwest::header::HeaderMap) {
        self.req_h(m, path, body, &[]).await
    }

    async fn req_h(&mut self, m: Method, path: &str, body: Option<Value>, extra: &[(&str, &str)]) -> (StatusCode, Value, reqwest::header::HeaderMap) {
        let mut rb = self.http.request(m.clone(), format!("{}{}", self.base, path));
        if let Some(c) = &self.cookie {
            rb = rb.header(header::COOKIE, c);
        }
        if m != Method::GET {
            if let Some(t) = &self.csrf {
                rb = rb.header("x-csrf-token", t);
            }
        }
        for (k, v) in extra {
            rb = rb.header(*k, *v);
        }
        if let Some(b) = body {
            rb = rb.json(&b);
        }
        let res = rb.send().await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        if let Some(sc) = headers.get(header::SET_COOKIE) {
            let s = sc.to_str().unwrap();
            let kv = s.split(';').next().unwrap().to_string();
            self.cookie = if kv.ends_with('=') { None } else { Some(kv) };
        }
        let text = res.text().await.unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        if let Some(t) = v.get("csrfToken").and_then(|t| t.as_str()) {
            self.csrf = Some(t.into());
        }
        (status, v, headers)
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

async fn boot(data: &std::path::Path) -> (Arc<AppState>, String) {
    let st = AppState::init(Options {
        data_dir: Some(data.to_path_buf()),
        secrets: Some(Arc::new(doto_core::secrets::MemoryStore::default())),
        skip_background_probes: true,
        ..Default::default()
    })
    .await
    .unwrap();
    let addr = st.start_control(0).await.unwrap();
    (st, format!("http://{addr}"))
}

async fn desktop(st: &AppState, base: &str) -> Client {
    let mut c = Client::new(base);
    let tok = st.issue_desktop_token();
    let (s, me, _) = c.req(Method::POST, "/api/auth/desktop", Some(json!({ "token": tok }))).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["hostControl"], true);
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auth_csrf_rbac_and_listener_isolation() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, base) = boot(tmp.path()).await;

    // chưa đăng nhập
    let mut anon = Client::new(&base);
    let (s, me, _) = anon.req(Method::GET, "/api/auth/me", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["user"], Value::Null);
    assert_eq!(me["setupRequired"], true);
    for p in ["/api/videos", "/api/skills", "/api/audit", "/api/system/hardware", "/api/tools"] {
        assert_eq!(anon.req(Method::GET, p, None).await.0, StatusCode::UNAUTHORIZED, "{p}");
    }

    // token desktop chỉ dùng một lần
    let tok = st.issue_desktop_token();
    let mut d = Client::new(&base);
    assert_eq!(d.req(Method::POST, "/api/auth/desktop", Some(json!({ "token": tok }))).await.0, StatusCode::OK);
    let mut d2 = Client::new(&base);
    assert_eq!(d2.req(Method::POST, "/api/auth/desktop", Some(json!({ "token": tok }))).await.0, StatusCode::UNAUTHORIZED);

    // CSRF: thiếu header → 403
    let saved = d.csrf.take();
    let (s, _, _) = d.req(Method::POST, "/api/system/backup", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    d.csrf = saved;
    // Origin lạ → 403
    let (s, _, _) = d.req_h(Method::POST, "/api/system/backup", None, &[("origin", "https://evil.example")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(d.req(Method::POST, "/api/system/backup", None).await.0, StatusCode::OK);

    // LAN bị chặn khi chưa đặt mật khẩu owner
    let port = free_port();
    let (s, e, _) = d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "lan", "port": port }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");

    // đặt mật khẩu yếu → 400; mạnh → ok
    assert_eq!(d.req(Method::POST, "/api/auth/setup", Some(json!({ "username": "admin", "password": "weak" }))).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(d.req(Method::POST, "/api/auth/setup", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await.0, StatusCode::OK);
    let (_, me, _) = d.req(Method::GET, "/api/auth/me", None).await;
    assert_eq!(me["setupRequired"], false);

    // bật public server localhost
    let (s, e, _) = d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "localhost", "port": port }))).await;
    assert_eq!(s, StatusCode::OK, "{e}");
    let (s, status, _) = d.req(Method::POST, "/api/server/start", None).await;
    assert_eq!(s, StatusCode::OK, "{status}");
    assert_eq!(status["running"], true);
    let pub_base = format!("http://127.0.0.1:{port}");

    // trùng cổng: listener khác chiếm cổng → lỗi rõ ràng
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = busy.local_addr().unwrap().port();
    d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "localhost", "port": busy_port }))).await;
    let (s, e, _) = d.req(Method::POST, "/api/server/restart", None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert!(e["message"].as_str().unwrap().contains("cổng"));
    drop(busy);
    d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "localhost", "port": port }))).await;
    assert_eq!(d.req(Method::POST, "/api/server/start", None).await.0, StatusCode::OK);

    // owner đăng nhập từ trình duyệt qua listener public: có đầy đủ quyền Owner như bản App,
    // nhưng /api/auth/desktop không tồn tại trên Public và /api/auth/setup bị khoá sau khi đã đặt mật khẩu
    let mut remote = Client::new(&pub_base);
    let (s, me, _) = remote.req(Method::POST, "/api/auth/login", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["hostControl"], false);
    assert_eq!(me["user"]["role"], "owner");
    assert_eq!(remote.req(Method::POST, "/api/auth/desktop", Some(json!({ "token": "x" }))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        remote.req(Method::POST, "/api/auth/setup", Some(json!({ "username": "admin2", "password": "Strong-pass-2026" }))).await.0,
        StatusCode::CONFLICT
    );

    // Owner trên bản Web dùng được đầy đủ chức năng trên phần cứng máy chủ: backup/list/restore, probe, onboarding, skill folder...
    let (s, bak, _) = remote.req(Method::POST, "/api/system/backup", None).await;
    assert_eq!(s, StatusCode::OK, "{bak}");
    let bak_file = bak["file"].as_str().unwrap().to_string();
    let (s, baks, _) = remote.req(Method::GET, "/api/system/backups", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(baks.as_array().unwrap().iter().any(|x| x["file"] == bak_file));
    // chặn path traversal khi restore
    assert_eq!(remote.req(Method::POST, "/api/system/restore", Some(json!({ "file": "../doto.sqlite", "confirm": true }))).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(remote.req(Method::POST, "/api/system/restore", Some(json!({ "file": bak_file, "confirm": true }))).await.0, StatusCode::OK);
    assert_eq!(remote.req(Method::POST, "/api/onboarding/complete", None).await.0, StatusCode::OK);
    assert_eq!(remote.req(Method::GET, "/api/tools/claude/diagnostics", None).await.0, StatusCode::OK);

    // chặn đặt ffmpegPath trỏ tới tệp không phải FFmpeg (ngăn thực thi binary tuỳ ý)
    let fake_bin = tmp.path().join(if cfg!(windows) { "cmd.exe" } else { "sh" });
    std::fs::write(&fake_bin, b"not ffmpeg").unwrap();
    let (_, mut bad_settings, _) = remote.req(Method::GET, "/api/settings", None).await;
    bad_settings["ffmpegPath"] = json!(fake_bin.to_string_lossy());
    assert_eq!(remote.req(Method::PUT, "/api/settings", Some(bad_settings)).await.0, StatusCode::BAD_REQUEST);

    // Cross-origin: chặn cookie CSRF, nhưng cho phép bản Web độc lập kết nối qua Bearer token + media ticket
    let (s, _, _) = remote.req_h(Method::POST, "/api/system/backup", None, &[("origin", "https://evil.example")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let preflight =
        reqwest::Client::new().request(Method::OPTIONS, format!("{pub_base}/api/auth/login")).header("Origin", "http://localhost:5173").send().await.unwrap();
    assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
    assert_eq!(preflight.headers().get("access-control-allow-origin").unwrap(), "http://localhost:5173");
    assert!(preflight.headers().get("access-control-allow-credentials").is_none(), "không bao giờ bật credentials cho cross-origin");

    let bearer_login = reqwest::Client::new()
        .post(format!("{pub_base}/api/auth/login"))
        .header("Origin", "http://localhost:5173")
        .json(&json!({ "username": "admin", "password": "Strong-pass-2026", "authMode": "bearer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bearer_login.status(), StatusCode::OK);
    let bearer_body: Value = bearer_login.json().await.unwrap();
    let bearer_tok = bearer_body["sessionToken"].as_str().unwrap().to_string();
    let ticket_res = reqwest::Client::new()
        .post(format!("{pub_base}/api/auth/ticket"))
        .header("Origin", "http://localhost:5173")
        .header("Authorization", format!("Bearer {bearer_tok}"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(ticket_res.status(), StatusCode::OK);
    let ticket = ticket_res.json::<Value>().await.unwrap()["ticket"].as_str().unwrap().to_string();
    // ticket chỉ được phép trên GET media/QR/SSE, không được phép gọi API quản trị
    let qr_ok = reqwest::get(format!("{pub_base}/api/server/qr?url=http%3A%2F%2F127.0.0.1%3A4380&ticket={ticket}")).await.unwrap();
    assert_eq!(qr_ok.status(), StatusCode::OK);
    let audit_denied = reqwest::get(format!("{pub_base}/api/audit?ticket={ticket}")).await.unwrap();
    assert_eq!(audit_denied.status(), StatusCode::UNAUTHORIZED);

    // cookie của listener Control không có hiệu lực trên Public
    let mut stolen = Client::new(&pub_base);
    stolen.cookie = d.cookie.clone();
    assert_eq!(stolen.req(Method::GET, "/api/audit", None).await.0, StatusCode::UNAUTHORIZED);

    // tạo viewer, viewer bị cấm tuyệt đối mọi route admin và điều khiển máy chủ
    let (s, _, _) = remote.req(Method::POST, "/api/users", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99", "role": "viewer" }))).await;
    assert_eq!(s, StatusCode::OK);
    let mut viewer = Client::new(&pub_base);
    assert_eq!(viewer.req(Method::POST, "/api/auth/login", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99" }))).await.0, StatusCode::OK);
    for p in
        ["/api/audit", "/api/skills", "/api/tools", "/api/devices", "/api/users", "/api/system/hardware", "/api/jobs", "/api/settings", "/api/tunnel/status"]
    {
        assert_eq!(viewer.req(Method::GET, p, None).await.0, StatusCode::FORBIDDEN, "viewer {p}");
    }
    for (m, p, body) in [
        (Method::POST, "/api/server/stop", json!({})),
        (Method::POST, "/api/tools/claude/install", json!({ "methodId": "npm", "confirm": true })),
        (Method::POST, "/api/skill-sources/folder", json!({ "path": "/tmp" })),
        (Method::PUT, "/api/settings", json!({})),
        (Method::POST, "/api/system/backup", json!({})),
        (Method::POST, "/api/system/restore", json!({ "file": "a.sqlite", "confirm": true })),
        (Method::POST, "/api/tunnel/start", json!({})),
        (Method::POST, "/api/videos/scan", json!({})),
    ] {
        assert_eq!(viewer.req(m.clone(), p, Some(body)).await.0, StatusCode::FORBIDDEN, "viewer {m} {p}");
    }
    assert_eq!(viewer.req(Method::POST, "/api/jobs/render", Some(json!({ "title": "x", "segments": [] }))).await.0, StatusCode::FORBIDDEN);
    let (s, v, _) = viewer.req(Method::GET, "/api/videos", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v.as_array().unwrap().len(), 0);

    // pairing: mã dùng một lần, revoke thiết bị
    let (s, p, _) = remote.req(Method::POST, "/api/pairing", Some(json!({ "role": "viewer", "ttlMinutes": 10 }))).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    let code = p["code"].as_str().unwrap().to_string();
    assert!(p["url"].as_str().unwrap().contains(&format!("pair={code}")));
    let mut phone = Client::new(&pub_base);
    let (s, me, _) = phone.req(Method::POST, "/api/auth/pair", Some(json!({ "code": code.to_lowercase(), "deviceName": "iPhone" }))).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["user"]["role"], "viewer");
    let mut phone2 = Client::new(&pub_base);
    assert_eq!(phone2.req(Method::POST, "/api/auth/pair", Some(json!({ "code": code }))).await.0, StatusCode::UNAUTHORIZED);
    let (_, devices, _) = remote.req(Method::GET, "/api/devices", None).await;
    let iphone = devices.as_array().unwrap().iter().find(|d| d["deviceName"] == "iPhone").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(phone.req(Method::GET, "/api/videos", None).await.0, StatusCode::OK);
    assert_eq!(remote.req(Method::DELETE, &format!("/api/devices/{iphone}"), None).await.0, StatusCode::OK);
    assert_eq!(phone.req(Method::GET, "/api/videos", None).await.0, StatusCode::UNAUTHORIZED, "thiết bị đã bị thu hồi");

    // token tunnel lưu vault, không bao giờ trả về
    let secret = "t".repeat(120);
    let (s, ts, _) = d.req(Method::PUT, "/api/tunnel/config", Some(json!({ "mode": "named", "hostname": "video.example.com", "token": secret }))).await;
    assert_eq!(s, StatusCode::OK, "{ts}");
    assert_eq!(ts["hasToken"], true);
    assert!(!ts.to_string().contains(&secret));
    let (_, audit, _) = d.req(Method::GET, "/api/audit?limit=500", None).await;
    assert!(!audit.to_string().contains(&secret), "audit không chứa token");
    assert!(!audit.to_string().contains("Strong-pass-2026"), "audit không chứa mật khẩu");

    // rate limit đăng nhập
    let mut brute = Client::new(&pub_base);
    let mut last = StatusCode::OK;
    for _ in 0..12 {
        last = brute.req(Method::POST, "/api/auth/login", Some(json!({ "username": "admin", "password": "wrong-pass-123" }))).await.0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);

    // security headers
    let (_, _, h) = anon.req(Method::GET, "/api/health", None).await;
    assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
    assert!(h.get("content-security-policy").is_some());

    assert_eq!(d.req(Method::POST, "/api/server/stop", None).await.0, StatusCode::OK);
    assert!(reqwest::get(format!("{pub_base}/api/health")).await.is_err(), "public server đã tắt");
    st.shutdown().await;
}

fn ffmpeg_ok() -> bool {
    let ok = which::which("ffmpeg").is_ok();
    if !ok && std::env::var("DOTO_REQUIRE_FFMPEG").as_deref() == Ok("1") {
        panic!("cần ffmpeg");
    }
    ok
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn video_e2e_render_stream_range_download_and_idor() {
    if !ffmpeg_ok() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("videos");
    std::fs::create_dir_all(&src).unwrap();
    let st = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=25:duration=3",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=3",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(src.join("nguồn.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let (state, base) = boot(&tmp.path().join("data")).await;
    let mut d = desktop(&state, &base).await;
    d.req(Method::POST, "/api/auth/setup", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await;

    let (_, mut settings, _) = d.req(Method::GET, "/api/settings", None).await;
    settings["videoSourceDirs"] = json!([src.to_string_lossy()]);
    let (s, e, _) = d.req(Method::PUT, "/api/settings", Some(settings.clone())).await;
    assert_eq!(s, StatusCode::OK, "{e}");
    let mut bad = settings.clone();
    bad["videoSourceDirs"] = json!(["relative/../x"]);
    assert_eq!(d.req(Method::PUT, "/api/settings", Some(bad)).await.0, StatusCode::BAD_REQUEST);

    let (s, scan, _) = d.req(Method::POST, "/api/videos/scan", None).await;
    assert_eq!(s, StatusCode::OK, "{scan}");
    assert_eq!(scan["addedOrUpdated"], 1);
    let (_, vids, _) = d.req(Method::GET, "/api/videos?kind=source", None).await;
    let source = vids[0].clone();
    assert!(source.get("filePath").is_none(), "không lộ đường dẫn");
    assert!(!vids.to_string().contains(&*src.to_string_lossy().replace('\\', "\\\\")));
    let source_id = source["id"].as_str().unwrap().to_string();

    // profile Eco → 1 job đồng thời; đổi profile không cần restart
    let (s, r, _) = d.req(Method::PUT, "/api/settings/resources", Some(json!({ "profile": "eco", "overrides": {} }))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["effective"]["maxJobs"], 1);

    let (s, job, _) = d
        .req(Method::POST, "/api/jobs/render", Some(json!({ "title": "Bản tin", "segments": [{ "videoId": source_id, "startMs": 500, "endMs": 2500 }], "encoder": "software", "visibility": "viewers" })))
        .await;
    assert_eq!(s, StatusCode::OK, "{job}");
    let job_id = job["id"].as_str().unwrap().to_string();
    let mut out_id = String::new();
    for _ in 0..300 {
        let (_, j, _) = d.req(Method::GET, &format!("/api/jobs/{job_id}"), None).await;
        if j["status"] == "succeeded" {
            out_id = j["outputVideoId"].as_str().unwrap().into();
            break;
        }
        assert_ne!(j["status"], "failed", "{j}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(!out_id.is_empty(), "render không hoàn tất");

    // public LAN-equivalent (localhost) + viewer
    let port = free_port();
    d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "localhost", "port": port }))).await;
    assert_eq!(d.req(Method::POST, "/api/server/start", None).await.0, StatusCode::OK);
    let pub_base = format!("http://127.0.0.1:{port}");
    let mut owner = Client::new(&pub_base);
    owner.req(Method::POST, "/api/auth/login", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await;
    owner.req(Method::POST, "/api/users", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99", "role": "viewer" }))).await;
    let mut viewer = Client::new(&pub_base);
    viewer.req(Method::POST, "/api/auth/login", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99" }))).await;

    let (_, vlist, _) = viewer.req(Method::GET, "/api/videos", None).await;
    let arr = vlist.as_array().unwrap();
    assert_eq!(arr.len(), 1, "viewer chỉ thấy export được chia sẻ");
    assert_eq!(arr[0]["id"], out_id.as_str());

    // IDOR: video nguồn private → 404 cho viewer
    for p in ["", "/stream", "/download", "/thumbnail"] {
        assert_eq!(viewer.req(Method::GET, &format!("/api/videos/{source_id}{p}"), None).await.0, StatusCode::NOT_FOUND, "{p}");
    }
    // path traversal trong id
    for bad in ["..%2F..%2Fdoto.sqlite", "%2e%2e%5cdoto.sqlite", "../../etc/passwd"] {
        let s = viewer.req(Method::GET, &format!("/api/videos/{bad}/stream"), None).await.0;
        assert!(s == StatusCode::NOT_FOUND || s == StatusCode::BAD_REQUEST, "{bad} → {s}");
    }

    // stream đầy đủ / range / 416
    let http = reqwest::Client::new();
    let cookie = viewer.cookie.clone().unwrap();
    let full = http.get(format!("{pub_base}/api/videos/{out_id}/stream")).header(header::COOKIE, &cookie).send().await.unwrap();
    assert_eq!(full.status(), StatusCode::OK);
    assert_eq!(full.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(full.headers()[header::CONTENT_TYPE], "video/mp4");
    let size = full.bytes().await.unwrap().len() as u64;
    assert!(size > 1000);
    let part =
        http.get(format!("{pub_base}/api/videos/{out_id}/stream")).header(header::COOKIE, &cookie).header(header::RANGE, "bytes=100-199").send().await.unwrap();
    assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.headers()[header::CONTENT_RANGE], format!("bytes 100-199/{size}").as_str());
    assert_eq!(part.bytes().await.unwrap().len(), 100);
    let tail =
        http.get(format!("{pub_base}/api/videos/{out_id}/stream")).header(header::COOKIE, &cookie).header(header::RANGE, "bytes=-50").send().await.unwrap();
    assert_eq!(tail.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(tail.bytes().await.unwrap().len(), 50);
    let bad = http
        .get(format!("{pub_base}/api/videos/{out_id}/stream"))
        .header(header::COOKIE, &cookie)
        .header(header::RANGE, format!("bytes={}-", size + 10))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(bad.headers()[header::CONTENT_RANGE], format!("bytes */{size}").as_str());

    // download
    let dl = http.get(format!("{pub_base}/api/videos/{out_id}/download")).header(header::COOKIE, &cookie).send().await.unwrap();
    assert_eq!(dl.status(), StatusCode::OK);
    let cd = dl.headers()[header::CONTENT_DISPOSITION].to_str().unwrap().to_string();
    assert!(cd.starts_with("attachment;") && cd.contains("filename*=UTF-8''B%E1%BA%A3n%20tin.mp4"), "{cd}");
    assert_eq!(dl.bytes().await.unwrap().len() as u64, size);
    // thumbnail
    let th = http.get(format!("{pub_base}/api/videos/{out_id}/thumbnail")).header(header::COOKIE, &cookie).send().await.unwrap();
    assert_eq!(th.status(), StatusCode::OK);
    assert_eq!(th.headers()[header::CONTENT_TYPE], "image/jpeg");

    // không đăng nhập → 401; share link có hạn → stream được, revoke → 404
    assert_eq!(reqwest::get(format!("{pub_base}/api/videos/{out_id}/stream")).await.unwrap().status(), StatusCode::UNAUTHORIZED);
    let (s, share, _) = owner.req(Method::POST, &format!("/api/videos/{out_id}/share"), Some(json!({ "ttlMinutes": 10 }))).await;
    assert_eq!(s, StatusCode::OK, "{share}");
    let url = share["url"].as_str().unwrap().to_string();
    let path = &url[url.find("/s/").unwrap()..];
    assert_eq!(reqwest::get(format!("{pub_base}{path}")).await.unwrap().status(), StatusCode::OK);
    owner.req(Method::DELETE, &format!("/api/shares/{}", share["id"].as_str().unwrap()), None).await;
    assert_eq!(reqwest::get(format!("{pub_base}{path}")).await.unwrap().status(), StatusCode::NOT_FOUND);

    // ẩn video khỏi viewer → viewer mất quyền ngay
    owner.req(Method::PATCH, &format!("/api/videos/{out_id}"), Some(json!({ "visibility": "private" }))).await;
    assert_eq!(viewer.req(Method::GET, &format!("/api/videos/{out_id}/stream"), None).await.0, StatusCode::NOT_FOUND);

    state.shutdown().await;
}

fn build_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = vec![];
    let mut central = vec![];
    for (name, data) in entries {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let off = out.len() as u32;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.sum().to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.sum().to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn make_cf_jwt(iss: &str, aud: &str, exp: i64) -> String {
    use doto_core::random_token;
    let b64 = |b: &[u8]| {
        // dùng hex/base64url không cần thêm dep ngoài: doto_server đã có base64 gián tiếp, ta mã hoá URL_SAFE_NO_PAD thủ công
        const ALPH: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut s = String::new();
        for chunk in b.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            s.push(ALPH[((n >> 18) & 63) as usize] as char);
            s.push(ALPH[((n >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                s.push(ALPH[((n >> 6) & 63) as usize] as char);
            }
            if chunk.len() > 2 {
                s.push(ALPH[(n & 63) as usize] as char);
            }
        }
        s
    };
    let hdr = b64(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload = b64(json!({ "iss": iss, "aud": [aud], "exp": exp }).to_string().as_bytes());
    let sig = random_token(24);
    format!("{hdr}.{payload}.{sig}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v02_studio_upload_archive_tunnels_cf_access_and_memory_parity() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, base) = boot(&tmp.path().join("data")).await;
    let mut d = desktop(&state, &base).await;
    assert_eq!(d.req(Method::POST, "/api/auth/setup", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await.0, StatusCode::OK);

    // Bật Public Server cho bản Web Owner + Viewer
    let port = free_port();
    assert_eq!(d.req(Method::PUT, "/api/server/config", Some(json!({ "mode": "localhost", "port": port }))).await.0, StatusCode::OK);
    assert_eq!(d.req(Method::POST, "/api/server/start", None).await.0, StatusCode::OK);
    let pub_base = format!("http://127.0.0.1:{port}");

    // Đăng nhập Web Owner (chế độ Bearer chéo miền) & tạo Viewer
    let http = reqwest::Client::new();
    let login_res: Value = http
        .post(format!("{pub_base}/api/auth/login"))
        .header("origin", "http://localhost:5173")
        .json(&json!({ "username": "admin", "password": "Strong-pass-2026", "authMode": "bearer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bearer = format!("Bearer {}", login_res["sessionToken"].as_str().unwrap());

    d.req(Method::POST, "/api/users", Some(json!({ "username": "viewer2", "password": "Viewer-pass-99", "role": "viewer" }))).await;
    let mut viewer = Client::new(&pub_base);
    viewer.req(Method::POST, "/api/auth/login", Some(json!({ "username": "viewer2", "password": "Viewer-pass-99" }))).await;

    // 1. P7: Host Directory Browser & mkdir từ Web Owner; Viewer bị chặn 403
    let dirs_res = http.get(format!("{pub_base}/api/fs/dirs")).header(header::AUTHORIZATION, &bearer).send().await.unwrap();
    assert_eq!(dirs_res.status(), StatusCode::OK);
    let dirs_json: Value = dirs_res.json().await.unwrap();
    assert!(!dirs_json["roots"].as_array().unwrap().is_empty());

    let browse_parent = tmp.path().join("host-browse");
    std::fs::create_dir_all(&browse_parent).unwrap();
    let mkdir_res = http
        .post(format!("{pub_base}/api/fs/mkdir"))
        .header(header::AUTHORIZATION, &bearer)
        .json(&json!({ "parent": browse_parent.to_string_lossy(), "name": "NewSub", "confirm": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(mkdir_res.status(), StatusCode::OK);
    assert!(browse_parent.join("NewSub").is_dir());

    // Chặn traversal tên thư mục & đường dẫn tương đối
    assert_eq!(
        http.get(format!("{pub_base}/api/fs/dirs?path=relative/dir")).header(header::AUTHORIZATION, &bearer).send().await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(viewer.req(Method::GET, "/api/fs/dirs", None).await.0, StatusCode::FORBIDDEN);

    // 2. P7: Streaming Mobile / App Photos Upload (ảnh PNG + từ chối .exe)
    let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4];
    let up_res = http
        .post(format!("{pub_base}/api/videos/upload?filename=iphone-photo.png"))
        .header(header::AUTHORIZATION, &bearer)
        .header(header::CONTENT_TYPE, "image/png")
        .body(png_bytes.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(up_res.status(), StatusCode::OK);
    let up_json: Value = up_res.json().await.unwrap();
    let upload_id = up_json["upload"]["id"].as_str().unwrap().to_string();
    assert_eq!(up_json["upload"]["mediaKind"], "image");

    let bad_ext = http
        .post(format!("{pub_base}/api/videos/upload?filename=malware.exe"))
        .header(header::AUTHORIZATION, &bearer)
        .body(b"MZ".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(bad_ext.status(), StatusCode::BAD_REQUEST);
    assert_eq!(viewer.req(Method::GET, "/api/uploads", None).await.0, StatusCode::FORBIDDEN);

    // 3. P8: Import Skill từ tệp .zip (chặn Zip-Slip) qua Web Owner
    let zip_bytes = build_test_zip(&[
        ("news-skill/SKILL.md", b"---\nname: Tin Nhanh Studio\ndescription: Tao video tin nhanh\n---\n# Huong dan\n"),
        ("../escaped.txt", b"hack"),
    ]);
    let arc_res = http
        .post(format!("{pub_base}/api/skill-sources/archive?filename=news-pack.zip&confirm=true"))
        .header(header::AUTHORIZATION, &bearer)
        .body(zip_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(arc_res.status(), StatusCode::OK);
    let arc_json: Value = arc_res.json().await.unwrap();
    assert_eq!(arc_json["imported"].as_array().unwrap().len(), 1);
    assert!(!arc_json["skipped"].as_array().unwrap().is_empty(), "Zip-Slip phải bị ghi nhận vào skipped");

    let skills_list: Value = http.get(format!("{pub_base}/api/skills")).header(header::AUTHORIZATION, &bearer).send().await.unwrap().json().await.unwrap();
    let imported_skill_id = skills_list[0]["id"].as_str().unwrap().to_string();

    // 4. P9: AI Studio chạy đa AI song song (Antigravity + Claude + Codex) trên phần cứng máy chủ
    #[cfg(windows)]
    let fake_cli = {
        let p = tmp.path().join("fake-studio-cli.cmd");
        std::fs::write(&p, "@echo off\r\necho {\"type\":\"result\",\"result\":\"Ban nhap AI Studio thanh cong\"}\r\n").unwrap();
        p
    };
    #[cfg(unix)]
    let fake_cli = {
        use std::os::unix::fs::PermissionsExt;
        let p = tmp.path().join("fake-studio-cli.sh");
        std::fs::write(&p, "#!/bin/sh\necho '{\"type\":\"result\",\"result\":\"Ban nhap AI Studio thanh cong\"}'\n").unwrap();
        let mut perms = std::fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms).unwrap();
        p
    };
    state.studio.set_binary_override("claude", fake_cli.clone());
    state.studio.set_binary_override("codex", fake_cli);

    let studio_res = http
        .post(format!("{pub_base}/api/studio/runs"))
        .header(header::AUTHORIZATION, &bearer)
        .json(&json!({
            "mode": "multi_ai",
            "prompt": "Dựng bản tin 30 giây từ ảnh đính kèm",
            "skillId": imported_skill_id,
            "sessions": [
                { "provider": "claude", "model": "claude-sonnet-4-6", "effort": "high" },
                { "provider": "codex", "model": "gpt-5.4", "effort": "medium" }
            ],
            "uploadIds": [upload_id],
            "includeMemoryHint": true,
            "confirm": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(studio_res.status(), StatusCode::OK);
    let studio_json: Value = studio_res.json().await.unwrap();
    let studio_job_id = studio_json["job"]["id"].as_str().unwrap().to_string();
    assert_eq!(studio_json["sessions"].as_array().unwrap().len(), 2);

    let mut studio_done = false;
    for _ in 0..50 {
        let r: Value =
            http.get(format!("{pub_base}/api/studio/runs/{studio_job_id}")).header(header::AUTHORIZATION, &bearer).send().await.unwrap().json().await.unwrap();
        if r["job"]["status"] == "succeeded" {
            studio_done = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(studio_done, "AI Studio đa phiên phải hoàn tất");
    assert_eq!(viewer.req(Method::GET, "/api/studio/runs", None).await.0, StatusCode::FORBIDDEN);

    // 5. P11: Shared Memory & Handoff 3 AI (lọc bí mật + chặn ngoài allowlist)
    let mem_dir = tmp.path().join("mem-workspace");
    std::fs::create_dir_all(&mem_dir).unwrap();
    let (_, mut cur_settings, _) = d.req(Method::GET, "/api/settings", None).await;
    cur_settings["workspaceMemoryDir"] = json!(mem_dir.to_string_lossy());
    assert_eq!(d.req(Method::PUT, "/api/settings", Some(cur_settings)).await.0, StatusCode::OK);

    assert_eq!(
        http.put(format!("{pub_base}/api/memory/doc"))
            .header(header::AUTHORIZATION, &bearer)
            .json(&json!({ "id": "MEMORY.md", "content": "Ghi chú chung ghp_secretLeakToken123456", "confirm": true }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let mem_doc: Value =
        http.get(format!("{pub_base}/api/memory/doc?id=MEMORY.md")).header(header::AUTHORIZATION, &bearer).send().await.unwrap().json().await.unwrap();
    assert!(!mem_doc["content"].as_str().unwrap().contains("ghp_secretLeak"));
    assert!(mem_doc["content"].as_str().unwrap().contains("[REDACTED]"));

    assert_eq!(
        http.put(format!("{pub_base}/api/memory/doc"))
            .header(header::AUTHORIZATION, &bearer)
            .json(&json!({ "id": "../outside.md", "content": "x", "confirm": true }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(viewer.req(Method::GET, "/api/memory", None).await.0, StatusCode::FORBIDDEN);

    // 6. P10: Multi-Tunnel CRUD + Cloudflare Access JWT enforcement
    let cf_tok = "z".repeat(80);
    let t_res = http
        .post(format!("{pub_base}/api/tunnels"))
        .header(header::AUTHORIZATION, &bearer)
        .json(&json!({
            "name": "Zero Trust Web",
            "kind": "token",
            "targetUrl": format!("http://127.0.0.1:{port}"),
            "hostname": "studio.example.com",
            "token": cf_tok
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(t_res.status(), StatusCode::OK);
    let t_body = t_res.text().await.unwrap();
    assert!(!t_body.contains(&cf_tok), "không lộ token tunnel trong HTTP response");

    // Bật Cloudflare Access trên listener Public
    assert_eq!(
        d.req(
            Method::PUT,
            "/api/server/config",
            Some(json!({
                "mode": "localhost",
                "port": port,
                "requireCfAccess": true,
                "cfAccessTeamDomain": "myteam.cloudflareaccess.com",
                "cfAccessAud": "aud-tag-1234567890"
            }))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(d.req(Method::POST, "/api/server/restart", None).await.0, StatusCode::OK);

    // Không có Cf-Access-Jwt-Assertion → 403 Forbidden
    assert_eq!(http.get(format!("{pub_base}/api/auth/me")).header(header::AUTHORIZATION, &bearer).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    // Sai AUD → 403 Forbidden
    let bad_jwt = make_cf_jwt("https://myteam.cloudflareaccess.com", "wrong-aud", chrono::Utc::now().timestamp() + 600);
    assert_eq!(
        http.get(format!("{pub_base}/api/auth/me"))
            .header(header::AUTHORIZATION, &bearer)
            .header("Cf-Access-Jwt-Assertion", bad_jwt)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    // Đúng ISS + AUD + chưa hết hạn → 200 OK
    let good_jwt = make_cf_jwt("https://myteam.cloudflareaccess.com", "aud-tag-1234567890", chrono::Utc::now().timestamp() + 600);
    assert_eq!(
        http.get(format!("{pub_base}/api/auth/me"))
            .header(header::AUTHORIZATION, &bearer)
            .header("Cf-Access-Jwt-Assertion", good_jwt)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    state.shutdown().await;
}

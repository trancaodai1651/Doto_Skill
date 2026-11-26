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

    // owner đăng nhập từ trình duyệt qua listener public: KHÔNG có hostControl, route [H] không tồn tại
    let mut remote = Client::new(&pub_base);
    let (s, me, _) = remote.req(Method::POST, "/api/auth/login", Some(json!({ "username": "admin", "password": "Strong-pass-2026" }))).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["hostControl"], false);
    for (m, p) in [
        (Method::POST, "/api/server/stop"),
        (Method::POST, "/api/tools/claude/install"),
        (Method::POST, "/api/skill-sources/folder"),
        (Method::PUT, "/api/settings"),
        (Method::POST, "/api/auth/desktop"),
        (Method::POST, "/api/tunnel/start"),
    ] {
        let (s, _, _) = remote.req(m.clone(), p, Some(json!({}))).await;
        assert!(s == StatusCode::NOT_FOUND || s == StatusCode::METHOD_NOT_ALLOWED, "{m} {p} → {s}");
    }
    // cookie của listener Control không có hiệu lực trên Public
    let mut stolen = Client::new(&pub_base);
    stolen.cookie = d.cookie.clone();
    assert_eq!(stolen.req(Method::GET, "/api/audit", None).await.0, StatusCode::UNAUTHORIZED);

    // tạo viewer, viewer bị cấm route admin
    let (s, _, _) = remote.req(Method::POST, "/api/users", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99", "role": "viewer" }))).await;
    assert_eq!(s, StatusCode::OK);
    let mut viewer = Client::new(&pub_base);
    assert_eq!(viewer.req(Method::POST, "/api/auth/login", Some(json!({ "username": "viewer1", "password": "Viewer-pass-99" }))).await.0, StatusCode::OK);
    for p in
        ["/api/audit", "/api/skills", "/api/tools", "/api/devices", "/api/users", "/api/system/hardware", "/api/jobs", "/api/settings", "/api/tunnel/status"]
    {
        assert_eq!(viewer.req(Method::GET, p, None).await.0, StatusCode::FORBIDDEN, "viewer {p}");
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

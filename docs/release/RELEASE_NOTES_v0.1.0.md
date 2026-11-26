# Doto Skill v0.1.0 — bản nền tảng

Bản phát hành đầu tiên của kiến trúc mới (Rust · Tauri v2 · Axum · React).

## Có gì
- Desktop + máy chủ web cục bộ: Off / Localhost / LAN, khay hệ thống, chạy nền, khởi động cùng OS (tuỳ chọn).
- Owner / Viewer, mã ghép nối thiết bị có hạn, thu hồi thiết bị, CSRF, rate limit, IP allowlist, nhật ký kiểm toán.
- Nhận diện phần cứng thật; 3 chế độ Nhẹ / Cân bằng / Tối đa ảnh hưởng hàng đợi render; probe encoder NVENC / Quick Sync / AMF / VideoToolbox bằng encode thử.
- Render video: trim/ghép → MP4 H.264/AAC, tiến độ/ETA thật, huỷ/thử lại, tự quay về libx264 nếu GPU lỗi.
- Thư viện web: phát/tua (HTTP Range), tải về, link chia sẻ có hạn (API).
- Skills: 0 skill mặc định; nhiều thư mục + repo GitHub (public; private qua gh CLI), xem diff trước khi cập nhật, pin, cài sang Claude Code / Codex / Antigravity.
- Quản lý 4 công cụ: Antigravity CLI, Codex CLI, Claude Code CLI, cloudflared (cài có xác nhận; cloudflared Windows được kiểm SHA-256 + chữ ký).
- Cloudflare Tunnel: Quick (thử nghiệm) và Named bằng token lưu trong OS vault.

## Hạn chế đã biết
- Installer **chưa ký số / chưa notarize** (macOS: chuột phải › Open; Windows: More info › Run anyway).
- GitHub Device Flow cần OAuth App client_id (chưa cấu hình) — dùng "gh CLI" cho repo private.
- Tunnel thật với tài khoản Cloudflare và thiết bị di động vật lý chưa được kiểm chứng trong phiên phát triển này.
- FFmpeg không đi kèm — cần cài riêng.

## Hệ thống yêu cầu
Windows 10 1809+/11 x64 (WebView2) · macOS 11+ · FFmpeg/ffprobe.

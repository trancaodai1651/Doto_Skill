# Doto Skill v0.1.0 — bản nền tảng

Bản phát hành đầu tiên của kiến trúc mới (Rust · Tauri v2 · Axum · React).

## Có gì
- **Bản App Desktop & Bản Web đầy đủ chức năng như nhau:** cửa sổ desktop (Tauri v2) + máy chủ web Axum cùng tiến trình (Off / Localhost / LAN, khay hệ thống, chạy nền, khởi động cùng OS); bản Web kết nối tới server của bản App (trực tiếp hoặc nhập URL máy chủ từ xa) để dùng phần cứng CPU/RAM/GPU của máy chủ với 100% tính năng quản trị cho Owner.
- **Bảo mật & phân quyền:** Owner / Viewer, mật khẩu Argon2id, mã ghép nối thiết bị có hạn + QR, thu hồi thiết bị, CSRF, CORS Bearer + media ticket ngắn hạn, rate limit, IP allowlist, nhật ký kiểm toán, sao lưu & khôi phục SQLite trực tuyến.
- **Phần cứng & hiệu năng:** nhận diện CPU/RAM/GPU/ổ đĩa/pin thật; 3 chế độ Nhẹ / Cân bằng / Tối đa ảnh hưởng hàng đợi render ngay lập tức; probe & benchmark 720p encoder NVENC / Quick Sync / AMF / VideoToolbox bằng encode thử.
- **Video E2E:** quét thư mục nguồn, metadata + thumbnail, trim/ghép → MP4 H.264/AAC, tiến độ/ETA thật, huỷ/thử lại, tự quay về libx264 nếu GPU lỗi; thư viện web phát/tua (HTTP Range 206), tải về, tạo & quản lý/thu hồi link chia sẻ có hạn.
- **Skills:** 0 skill mặc định; nhiều thư mục + repo GitHub (public; private qua gh CLI / Device Flow), xem diff trước khi cập nhật, pin commit, cài sang Claude Code / Codex / Antigravity.
- **AI CLI & Cloudflare Tunnel:** quản lý 4 công cụ Antigravity CLI, Codex CLI, Claude Code CLI, cloudflared (allowlist cài đặt có xác nhận; cloudflared Windows kiểm SHA-256 + chữ ký Authenticode); Quick Tunnel và Named Tunnel (token lưu trong OS vault).

## Hạn chế đã biết
- Installer **chưa ký số / chưa notarize** (macOS: chuột phải › Open; Windows: More info › Run anyway).
- GitHub Device Flow cần OAuth App client_id (chưa cấu hình) — dùng "gh CLI" cho repo private.
- Tunnel thật với tài khoản Cloudflare và thiết bị di động vật lý chưa được kiểm chứng trong phiên phát triển này.
- FFmpeg không đi kèm — cần cài riêng.

## Hệ thống yêu cầu
Windows 10 1809+/11 x64 (WebView2) · macOS 11+ · FFmpeg/ffprobe.

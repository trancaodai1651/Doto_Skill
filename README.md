# Doto Skill

Ứng dụng desktop **Windows & macOS** đồng thời là **máy chủ web cục bộ**: quản lý nhiều nguồn AI Skills, cài đặt có xác nhận Google Antigravity CLI · OpenAI Codex CLI · Claude Code CLI · Cloudflare Tunnel, nhận diện phần cứng với 3 chế độ tài nguyên, render video bằng FFmpeg và chia sẻ thư viện video cho điện thoại/máy tính bảng qua trình duyệt.

> Trạng thái: **v0.1.0 — bản nền tảng**. Xem chức năng đã kiểm chứng / còn chờ trong [`docs/shared/TASK_BOARD.md`](docs/shared/TASK_BOARD.md).

## Tính năng
- **Bản App & Bản Web đồng nhất 100% tính năng:** cửa sổ desktop (Tauri v2) + máy chủ Axum chạy cùng tiến trình; bản Web kết nối tới máy chủ của bản App (cùng địa chỉ LAN/Tunnel hoặc cấu hình URL máy chủ từ xa) để dùng trực tiếp phần cứng CPU/RAM/GPU của máy chủ với đầy đủ tính năng như bản App. Hỗ trợ bật/tắt/khởi động lại web server, chế độ Off / Localhost / LAN, QR, chạy nền ở khay hệ thống, khởi động cùng hệ điều hành.
- **Bảo mật:** Owner / Viewer, mật khẩu Argon2id, cookie HttpOnly + SameSite=Strict + CSRF (hoặc Bearer + media ticket ngắn hạn khi kết nối chéo miền), rate limit, mã ghép nối thiết bị hết hạn, thu hồi thiết bị, IP allowlist, nhật ký kiểm toán. Không có API chạy lệnh shell hay đọc ổ đĩa tuỳ ý; Viewer chỉ xem/tải video được chia sẻ.
- **Cloudflare Tunnel:** Quick Tunnel (thử nghiệm) và Named Tunnel bằng token (lưu trong Keychain/Credential Manager), không mở cổng router.
- **AI CLI:** dò tìm, phiên bản, trạng thái đăng nhập, cài từ nguồn chính thức theo allowlist sau khi xác nhận lệnh, chẩn đoán.
- **Phần cứng & hiệu năng:** CPU/RAM/GPU/ổ đĩa/pin thật; probe & benchmark 720p encoder NVENC / Quick Sync / AMF / VideoToolbox bằng encode thử; profile **Nhẹ / Cân bằng / Tối đa** thay đổi số job, luồng FFmpeg và ngưỡng tải ngay lập tức.
- **Skills:** khởi đầu **0 skill**; thêm không giới hạn thư mục và repo GitHub (public/private), theo dõi thay đổi, xem diff trước khi cập nhật, pin commit, cài sang Claude / Codex / Antigravity theo đúng thư mục chính thức. Skill là dữ liệu không tin cậy — không bao giờ tự chạy script.
- **Video & dữ liệu:** quét thư mục, metadata + thumbnail, trim/ghép nhiều đoạn → MP4 H.264/AAC (GPU khi có), tiến độ/ETA thật, huỷ/thử lại; thư viện web responsive với phát/tua (HTTP Range), tải về có kiểm soát quyền, danh sách & thu hồi link chia sẻ có hạn; sao lưu & khôi phục cơ sở dữ liệu SQLite trực tuyến.

## Cài đặt (người dùng)
Tải installer tại trang [Releases](https://github.com/trancaodai1651/Doto_Skill/releases) và đối chiếu `SHA256SUMS.txt`:
- Windows 10/11 x64: `Doto Skill_<ver>_x64-setup.exe` (NSIS, không cần admin) hoặc `.msi`.
- macOS 11+ (Intel & Apple Silicon): `.dmg` (universal). Bản chưa notarize — xem [docs/USER_GUIDE.md](docs/USER_GUIDE.md#macos-gatekeeper).

Yêu cầu thêm: **FFmpeg + ffprobe** (không đóng gói kèm vì giấy phép) — Windows `winget install Gyan.FFmpeg`, macOS `brew install ffmpeg`, hoặc trỏ đường dẫn trong Cài đặt.

## Phát triển
```
npm ci
npm run build -w apps/web
npm run dev:server        # API + UI: http://127.0.0.1:4319 (mở URL desktop-token được in ra)
npm run dev:web           # Vite dev server, proxy /api → 4319
npm run desktop:dev       # cửa sổ Tauri
```
Kiểm thử & đóng gói:
```
npm run format | lint | typecheck | unit-test | integration-test | e2e | build | release-check
npm run desktop:build     # installer cho OS hiện tại
```
Kiến trúc: [`docs/shared/PROJECT_MEMORY.md`](docs/shared/PROJECT_MEMORY.md) · API: [`docs/API.md`](docs/API.md) · Tích hợp: [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) · Phát hành: [`docs/RELEASE.md`](docs/RELEASE.md) · Bảo mật: [`docs/SECURITY.md`](docs/SECURITY.md).

## Dành cho AI agent
Claude Code đọc `CLAUDE.md`, Codex đọc `AGENTS.md`, Antigravity đọc `.agents/rules/` + `GEMINI.md`. Tất cả cùng dùng `docs/shared/` và `docs/agents/HANDOFF_*.md`.

## Giấy phép
Chưa chọn giấy phép mã nguồn — chủ sở hữu repo cần quyết định (xem TASK_BOARD). Danh sách giấy phép phụ thuộc được sinh bởi `node scripts/license-check.mjs`.

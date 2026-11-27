# Doto Skill v0.2.0 — AI Studio Đa Phiên, Mobile Uploads & CRUD Đa Tunnel

Bản nâng cấp lớn đưa toàn bộ hệ sinh thái **Studio nâng cao (P7 → P13)** lên nền tảng **Rust · Tauri v2 · Axum · React 19**, giữ nguyên nguyên tắc **đồng nhất 100% tính năng giữa bản Desktop App và bản Web** (bản Web kết nối và sử dụng trực tiếp phần cứng CPU/RAM/GPU của máy chủ bản App qua API allowlist an toàn).

## Tính năng mới trong v0.2.0
- **Duyệt thư mục máy chủ an toàn & Tải media từ Điện thoại / App Photos (P7):**
  - Trình chọn thư mục trực quan ngay trên UI cho cả bản App lẫn bản Web (`/api/fs/dirs`, `/api/fs/mkdir`, chỉ liệt kê thư mục con, không theo symlink, không lộ nội dung tệp).
  - Tải ảnh/video trực tiếp từ iPhone, iPad, Android hoặc máy tính lên ổ đĩa máy chủ (stream tới 1 GB, tự động lập chỉ mục video vào **Video nguồn** kèm metadata `ffprobe` + thumbnail).
  - Nút **Lưu về App Photos / Chia sẻ** trên trang phát video sử dụng Web Share API Level 2 (`navigator.share({ files })`).
- **Import Skill từ tệp nén & tải thư mục từ trình duyệt (P8):**
  - Giải nén an toàn `.zip` (thuần Rust + kiểm tra CRC-32), `.tar.gz`/`.tgz`, `.tar`, và `.7z`/`.rar` (qua `tar` hệ thống kèm quét dọn symlink & kiểm tra biên đường dẫn).
  - Tải cả thư mục skill từ trình duyệt (`webkitdirectory`), hỗ trợ tuỳ chọn ghi đè và tự sinh `SKILL.md` tối thiểu cho thư mục tài nguyên/prompt; tuyệt đối không tự thực thi script.
- **AI Studio đơn phiên & đa phiên song song (P9):**
  - Điều phối chạy **Đơn phiên (1 AI)**, **Đa phiên cùng 1 AI (2–6 phương án)** hoặc **Đa AI song song** (**Google Antigravity `agy` · Claude Code `claude` · OpenAI Codex `codex`**) trên phần cứng máy chủ.
  - Mọi tiến trình chạy bằng mảng đối số cố định (không qua shell), bắt buộc Owner xác nhận lệnh trước khi chạy, tách riêng thư mục xuất cô lập từng phiên (`studio-<id>-s1`, `-s2`...), phân tích `stream-json` trực tiếp qua SSE, hỗ trợ dừng từng phiên hoặc dừng toàn bộ, và tự động lập chỉ mục video MP4 mới xuất vào Thư viện.
- **Trình quản lý CRUD đa Cloudflare Tunnel & Cloudflare Access (P10):**
  - Quản lý đồng thời nhiều cấu hình Quick Tunnel (`*.trycloudflare.com`), Zero Trust Token (lưu riêng từng tunnel trong OS Keychain / Credential Manager, truyền qua `TUNNEL_TOKEN` env) và Named Tunnel; nút mở nhanh 1-click Quick Web Tunnel; phát hiện và dừng an toàn tiến trình `cloudflared` bên ngoài.
  - Tuỳ chọn bắt buộc xác thực **Cloudflare Access** (`Cf-Access-Jwt-Assertion`: kiểm tra kết nối loopback `cloudflared` + claims `iss`/`aud`/`exp`) trên listener Public.
- **Trung tâm Bộ nhớ chung & Bàn giao 3 AI trên giao diện (P11):**
  - Đọc, chỉnh sửa, khởi tạo tệp mẫu và ghi bàn giao liên phiên trực tiếp cho `PROJECT_MEMORY.md`, `ROADMAP.md`, `TASK_BOARD.md`, `DECISIONS.md`, `HANDOFF_ANTIGRAVITY.md`, `HANDOFF_CLAUDE.md`, `HANDOFF_CODEX.md`, `MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` (tự động che `[REDACTED]` mọi chuỗi giống token/bí mật trước khi lưu).
- **Playwright E2E trên binary `doto-server` thật trong CI (P12):**
  - Bổ sung kiểm thử E2E trình duyệt thực thi toàn bộ vòng đời (Setup → Onboarding → Import `.zip` → Memory Handoff → Mobile Upload → Public Web Owner → Ghép nối Viewer RBAC) trực tiếp trên tiến trình `doto-server` biên dịch thật.

## Hạn chế đã biết
- Installer **chưa ký số / chưa notarize** (macOS: chuột phải › Open; Windows: More info › Run anyway) cho tới khi cấu hình chứng chỉ Apple Developer / Windows Authenticode.
- GitHub Device Flow cần `DOTO_GITHUB_CLIENT_ID` (dùng tuỳ chọn "Dùng gh CLI" cho repo private khi chưa đặt).
- FFmpeg không đi kèm bộ cài — cài qua `winget install Gyan.FFmpeg` (Windows) hoặc `brew install ffmpeg` (macOS).

## Hệ thống yêu cầu
Windows 10 1809+/11 x64 (WebView2) · macOS 11+ (Universal: Intel & Apple Silicon) · FFmpeg/ffprobe.

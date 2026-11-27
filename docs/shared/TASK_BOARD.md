# TASK_BOARD — Doto Skill

> Owner agent: `claude` · `codex` · `antigravity`. Chỉ chuyển sang Done khi có bằng chứng (lệnh + kết quả trong `LAST_TEST_RUN.md` hoặc CI).

## In progress
| Task | Owner | Ghi chú / next action |
| --- | --- | --- |
| _(Không có — `v0.2.0` hoàn tất P0–P13)_ | — | Xem mục **Pending** bên dưới cho kiểm thử thiết bị vật lý & chứng chỉ |

## Pending
| Task | Owner | Ghi chú |
| --- | --- | --- |
| Đăng ký GitHub OAuth App, cấu hình `DOTO_GITHUB_CLIENT_ID` cho Device Flow | chủ repo | Blocker B1 — cần tài khoản GitHub của chủ sở hữu |
| Kiểm chứng named tunnel thật (token Zero Trust + hostname) và Cloudflare Access trên domain sản xuất | chủ repo | Blocker B2 — code CRUD đa tunnel + kiểm tra `Cf-Access-Jwt-Assertion` đã xong & có test tự động |
| Kiểm chứng trên điện thoại/tablet vật lý cùng LAN (upload App Photos & Web Share Level 2) | chủ repo | Đã kiểm chứng tự động qua Playwright E2E trên `doto-server` thật |
| Chạy installer macOS trên máy Mac thật (Intel + Apple Silicon) | bất kỳ | CI build universal (`lipo` x86_64 + arm64); cần người kiểm thử trên phần cứng Mac |
| Ký số Authenticode / Apple notarization | chủ repo | Cần chứng chỉ (Blocker B3) |

## Blocked
| ID | Mô tả | Cách gỡ |
| --- | --- | --- |
| B1 | Device Flow GitHub cần OAuth App client_id | Tạo OAuth App (Settings › Developer settings › OAuth Apps, bật Device Flow) → đặt biến môi trường `DOTO_GITHUB_CLIENT_ID` khi chạy app/CI build |
| B2 | Tunnel production cần tài khoản Cloudflare + domain | Tạo tunnel trên Zero Trust, public hostname → `http://127.0.0.1:<port>`, dán token vào app |
| B3 | Ký mã/notarize | Thêm secrets `APPLE_*` cho release workflow; chứng chỉ Windows |

## Done (v0.2.0 — 2026-11-27, antigravity)
- **P7 — Duyệt thư mục máy chủ & Mobile/App Photos Media:** API `/api/fs/dirs`, `/api/fs/mkdir` (chỉ liệt kê thư mục con, không theo symlink, chặn Viewer `403`), `<FolderPickerModal />` dùng chung cho cả App & Web, stream upload ảnh/video tới 1 GB (`POST /api/videos/upload` → tự lập chỉ mục `video_assets`) và nút **Lưu về App Photos / Chia sẻ** (`navigator.share({ files })`).
- **P8 — Import Skill từ tệp nén & tải thư mục trình duyệt:** bộ giải nén `.zip` thuần Rust (kiểm CRC-32, chặn Zip-Slip & zip bomb) + `.tar.gz`/`.tar` + fallback `tar` hệ thống (`.7z`/`.rar` kèm dọn symlink), tải thư mục `webkitdirectory` (`POST /api/skill-sources/upload-folder`), tuỳ chọn tự tạo `SKILL.md` tối thiểu, không bao giờ tự chạy script.
- **P9 — AI Studio đơn phiên & đa phiên song song:** `StudioRunner` (`crates/integrations/src/studio.rs`) + trang `/studio` chạy `single`, `multi_same_ai`, `multi_ai` (`Antigravity` `agy`, `Claude Code` `claude`, `OpenAI Codex` `codex`) bằng đối số cố định không qua shell, truyền prompt nhiều dòng qua `stdin` (chống lỗi `.cmd` CVE-2024-24576 trên Windows), thư mục xuất cô lập từng phiên, phân tích `stream-json` trực tiếp qua SSE, huỷ từng phiên/toàn bộ và tự lập chỉ mục video MP4 mới xuất.
- **P10 — CRUD đa Cloudflare Tunnel & Cloudflare Access:** bảng `managed_tunnels` + OS Vault riêng từng tunnel (`cloudflare.tunnel.<id>`, truyền qua env `TUNNEL_TOKEN`), nút mở nhanh Quick Web Tunnel, dò & dừng an toàn tiến trình `cloudflared` ngoài, và xác thực JWT `Cf-Access-Jwt-Assertion` (`iss`, `aud`, `exp`, loopback `cloudflared`) trên listener Public.
- **P11 — Trung tâm Shared Memory & Handoff 3 AI trên UI:** API `/api/memory/*` + trang `/memory` quản lý allowlist tệp điều phối (`PROJECT_MEMORY.md`, `ROADMAP.md`, `TASK_BOARD.md`, `DECISIONS.md`, `HANDOFF_*.md`, `MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`), tự lọc `[REDACTED]` trước khi ghi đĩa.
- **P12 & P13 — Kiểm thử toàn diện, E2E `doto-server` thật trong CI & Release `v0.2.0` (`cef4894`):** 115 test Rust + 33 unit test Web + 5 kịch bản Playwright E2E mock + 1 kịch bản Playwright E2E toàn trình trên binary `doto-server` thật trong CI (`45919102697`); nâng phiên bản `0.2.0` và phát hành thành công GitHub Release `v0.2.0` (`45919537257`: `.exe` NSIS, `.msi`, universal `.dmg`, `.app.zip`, `SHA256SUMS.txt`, `THIRD_PARTY_LICENSES.tsv` tại https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.2.0).

## Done (v0.1.0 — 2026-11-26, claude + antigravity)
- Monorepo + 6 crate Rust + Tauri desktop + web React; clippy `-D warnings`, rustfmt.
- **Đồng bộ đầy đủ 100% tính năng giữa bản App và bản Web cho Owner (`ADR-013`):** bản Web kết nối tới máy chủ của bản App (trực tiếp hoặc nhập địa chỉ máy chủ qua `ServerConnectionBox`, hỗ trợ CORS Bearer + vé media ngắn hạn `POST /api/auth/ticket`), sử dụng phần cứng CPU/RAM/GPU của máy chủ; Viewer vẫn bị khóa chặt RBAC `403`/`404`.
- **Bảo mật bổ sung & chống lỗi vận hành:** xác minh binary `ffmpeg`/`ffprobe` thật trước khi lưu cấu hình đường dẫn; khóa `/api/auth/setup` sau lần đầu và chỉ cho phép trên loopback; tách tác vụ restart/stop listener Public tránh tự khóa (self-deadlock); sửa lỗi crash UI khi `cloudflared` phát sự kiện log.
- SQLite migrations + sao lưu & khôi phục trực tuyến (`GET /api/system/backups`, `POST /api/system/backup`, `POST /api/system/restore` + UI Cài đặt); settings; audit (lọc bí mật); jobs; OS vault.
- HardwareProbe thật (CPU/RAM/GPU qua CIM + nvidia-smi/system_profiler, disk + gợi ý ổ đĩa máy chủ trên Web, pin); ResourceManager 3 profile + admission.
- FFmpeg discover/verify/license flags; probe encoder thật + benchmark tốc độ 720p thực tế (`POST /api/system/ffmpeg/benchmark`); render trim/concat; cancel/retry/fallback/recovery.
- Skills: scanner an toàn, nhiều thư mục + watcher, GitHub public import thật, check-update diff, pin, mapping 3 provider.
- 4 CliProvider + ToolManager (detect/version/auth/install có xác nhận/diagnostics); cloudflared installer có kiểm checksum + Authenticode; TunnelManager quick/named.
- Server: auth/CSRF/CORS Bearer/RBAC/pairing/revoke/rate-limit/IP allowlist/security headers; Range/416/download; tạo + liệt kê + thu hồi share link (`GET/POST /api/videos/:id/shares|share`, `DELETE /api/shares/:id`); SSE.
- Kiểm thử: 101 test Rust + 33 unit test Web + 4 kịch bản Playwright E2E + secrets-scan + license-check xanh 100%.
- **Phát hành GitHub Release `v0.1.0` (`12084d4677`):** CI + Release workflow (`45788563182`) xanh trên Ubuntu, Windows x64 và macOS universal (`x86_64` + `arm64`); đã upload `.exe` NSIS, `.msi`, `.dmg`, `.app.zip`, `SHA256SUMS.txt`, `THIRD_PARTY_LICENSES.tsv` lên https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.1.0.

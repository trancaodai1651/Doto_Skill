# TASK_BOARD — Doto Skill

> Owner agent: `claude` · `codex` · `antigravity`. Chỉ chuyển sang Done khi có bằng chứng (lệnh + kết quả trong `LAST_TEST_RUN.md` hoặc CI).

## In progress
| Task | Owner | Ghi chú / next action |
| --- | --- | --- |
| _(Không có — `v0.1.0` đã phát hành trên `main`)_ | — | Xem mục **Pending** bên dưới cho v0.2.0 |

## Pending
| Task | Owner | Ghi chú |
| --- | --- | --- |
| Đăng ký GitHub OAuth App, cấu hình `DOTO_GITHUB_CLIENT_ID` cho Device Flow | chủ repo | Blocker B1 — cần tài khoản GitHub của chủ sở hữu |
| Kiểm chứng named tunnel thật (token Zero Trust + hostname) và Quick Tunnel trên máy người dùng | bất kỳ | Blocker B2 — cần tài khoản Cloudflare; code + test đơn vị + fix SSE tunnel đã có |
| Kiểm chứng trên điện thoại/tablet vật lý cùng LAN | chủ repo | Hiện kiểm chứng bằng trình duyệt qua IP LAN/Tailscale cùng host |
| Chạy installer macOS trên máy Mac thật (Intel + Apple Silicon) | bất kỳ | CI build universal (`lipo` x86_64 + arm64); cần người kiểm thử trên phần cứng Mac |
| Ký số Authenticode / Apple notarization | chủ repo | Cần chứng chỉ (Blocker B3) |
| Playwright E2E chạy trực tiếp trên binary `doto-server` trong CI | codex | Dùng `doto-server --token-file` |

## Blocked
| ID | Mô tả | Cách gỡ |
| --- | --- | --- |
| B1 | Device Flow GitHub cần OAuth App client_id | Tạo OAuth App (Settings › Developer settings › OAuth Apps, bật Device Flow) → đặt biến môi trường `DOTO_GITHUB_CLIENT_ID` khi chạy app/CI build |
| B2 | Tunnel production cần tài khoản Cloudflare + domain | Tạo tunnel trên Zero Trust, public hostname → `http://127.0.0.1:<port>`, dán token vào app |
| B3 | Ký mã/notarize | Thêm secrets `APPLE_*` cho release workflow; chứng chỉ Windows |

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

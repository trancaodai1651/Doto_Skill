# TASK_BOARD — Doto Skill

> Owner agent: `claude` · `codex` · `antigravity`. Chỉ chuyển sang Done khi có bằng chứng (lệnh + kết quả trong `LAST_TEST_RUN.md` hoặc CI).

## In progress
| Task | Owner | Ghi chú / next action |
| --- | --- | --- |
| CI GitHub xanh trên Windows + macOS cho PR `feat/initial-platform` | claude | Theo dõi workflow CI; sửa nếu runner macOS lỗi |
| Release v0.1.0 (installer Win NSIS/MSI + macOS universal DMG/app.zip + SHA256SUMS) | claude | Chỉ tag khi CI xanh; bản unsigned, ghi chú Gatekeeper/SmartScreen |

## Pending
| Task | Owner | Ghi chú |
| --- | --- | --- |
| Đăng ký GitHub OAuth App, cấu hình `DOTO_GITHUB_CLIENT_ID` cho Device Flow | chủ repo | Blocker B1 — cần tài khoản GitHub của chủ sở hữu |
| Kiểm chứng named tunnel thật (token Zero Trust + hostname) và Quick Tunnel trên máy người dùng | bất kỳ | Blocker B2 — cần tài khoản Cloudflare; code + test đơn vị đã có |
| Kiểm chứng trên điện thoại/tablet vật lý cùng LAN | chủ repo | Hiện kiểm chứng bằng trình duyệt qua IP Tailscale cùng host |
| Chạy installer macOS trên máy Mac thật (Intel + Apple Silicon) | bất kỳ | CI build universal; cần người kiểm thử |
| Ký số Authenticode / Apple notarization | chủ repo | Cần chứng chỉ |
| Playwright E2E chạy trên server Rust thật trong CI | codex | Dùng `doto-server --token-file` |

## Blocked
| ID | Mô tả | Cách gỡ |
| --- | --- | --- |
| B1 | Device Flow GitHub cần OAuth App client_id | Tạo OAuth App (Settings › Developer settings › OAuth Apps, bật Device Flow) → đặt biến môi trường `DOTO_GITHUB_CLIENT_ID` khi chạy app/CI build |
| B2 | Tunnel production cần tài khoản Cloudflare + domain | Tạo tunnel trên Zero Trust, public hostname → `http://127.0.0.1:<port>`, dán token vào app |
| B3 | Ký mã/notarize | Thêm secrets `APPLE_*` cho release workflow; chứng chỉ Windows |

## Done (v0.1.0 nền tảng — 2026-11-26, claude)
- Monorepo + 6 crate Rust + Tauri desktop + web React; clippy `-D warnings`, rustfmt.
- SQLite migrations/backup/restore; settings; audit (lọc bí mật); jobs; OS vault.
- HardwareProbe thật (CPU/RAM/GPU qua CIM + nvidia-smi/system_profiler, disk, pin); ResourceManager 3 profile + admission.
- FFmpeg discover/verify/license flags; probe encoder thật (NVENC ✔ trên máy dev); render trim/concat; cancel/retry/fallback/recovery.
- Skills: scanner an toàn, nhiều thư mục + watcher, GitHub public import thật, check-update diff, pin, mapping 3 provider.
- 4 CliProvider + ToolManager (detect/version/auth/install có xác nhận/diagnostics); cloudflared installer có kiểm checksum + Authenticode; TunnelManager quick/named.
- Server: auth/CSRF/RBAC/pairing/revoke/rate-limit/IP allowlist/security headers; Range/416/download; share link; SSE.
- Demo E2E thật trong trình duyệt: onboarding → LAN → scan → render NVENC → Viewer qua URL LAN xem/tua/tải.
- Scripts: task runner, update-handoff, secrets-scan, license-check; workflow CI + Release.
- UI link chia sẻ có hạn trong trang phát video (Owner, video export): tạo có xác nhận, sao chép, thu hồi — kiểm chứng trên server thật (206 ẩn danh → thu hồi → 404).
- Installer Windows build cục bộ: NSIS 5.51 MiB + MSI 7.82 MiB, smoke test binary release.

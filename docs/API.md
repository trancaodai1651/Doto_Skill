# Doto Skill — HTTP API (v1)

Kiểu dữ liệu: `apps/web/src/api/types.ts` (nguồn sự thật cho client) ↔ `crates/server/src/dto.rs` (serde `camelCase`).

## Mô hình listener & quyền

| Listener | Bind | Ai dùng | Quyền tối đa |
| --- | --- | --- | --- |
| **Control** (nội bộ) | `127.0.0.1:<cổng ngẫu nhiên>` luôn bật khi app chạy | Cửa sổ desktop (Tauri) | Owner + `hostControl` |
| **Public web** | Off / `127.0.0.1:<port>` / LAN `0.0.0.0` hoặc IP interface | Trình duyệt thiết bị khác, cloudflared | Owner (không `hostControl`) / Viewer |

- `hostControl` chỉ cấp cho phiên tạo qua `POST /api/auth/desktop` trên listener Control bằng **token một lần** mà app desktop sinh ra mỗi lần khởi động. Các route `[H]` không tồn tại trên listener Public (trả 404) → trình duyệt từ xa **không bao giờ** cài phần mềm, chạy CLI, đổi thư mục root hay đọc filesystem host.
- Xác thực: cookie `doto_session` (HttpOnly, SameSite=Strict, `Secure` khi request đến qua HTTPS/`X-Forwarded-Proto: https`) hoặc `Authorization: Bearer <token>`.
- CSRF: mọi request thay đổi trạng thái dùng cookie phải gửi header `x-csrf-token` (lấy từ `GET /api/auth/me`).
- Rate limit đăng nhập/ghép nối: 10 lần/5 phút/IP. IP allowlist (CIDR) áp dụng cho listener Public.
- Lỗi: `{ "error": "<code>", "message": "<tiếng Việt>" }` với 400/401/403/404/409/413/416/429/500.

Ký hiệu: `[P]` public không cần đăng nhập · `[V]` Viewer trở lên · `[O]` Owner · `[H]` Owner + hostControl (chỉ listener Control).

## Auth & người dùng
- `GET  /api/health` [P] → `{ ok, version, uptimeSeconds }`
- `GET  /api/auth/me` [P] → `Me`
- `POST /api/auth/setup` [H] `{ username, password }` → tạo Owner đầu tiên (mật khẩu ≥ 10 ký tự, có chữ & số)
- `POST /api/auth/desktop` [P, chỉ Control] `{ token }` → phiên Owner + hostControl
- `POST /api/auth/login` [P] `{ username, password, deviceName? }` → `Me`
- `POST /api/auth/pair` [P] `{ code, deviceName }` → phiên theo role của mã ghép nối
- `POST /api/auth/logout` [V]
- `GET/POST /api/users` [O] · `DELETE /api/users/:id` [O] · `POST /api/users/:id/password` [O]
- `GET /api/devices` [O] → `DeviceSession[]` · `DELETE /api/devices/:id` [O] (thu hồi)
- `POST /api/pairing` [O] `{ role, ttlMinutes }` → `PairingCode`

## Hệ thống & tài nguyên
- `GET /api/system/hardware` [O] → `HardwareInfo` · `POST /api/system/hardware/probe` [H]
- `GET /api/system/metrics` [O] → `Metrics`
- `GET /api/system/ffmpeg` [O] → `FfmpegStatus` · `POST /api/system/ffmpeg/probe` [H] (dò lại + encode thử)
- `GET /api/settings/resources` [O] → `ResourceSettings` · `PUT` [O] `{ profile, overrides }` (áp dụng ngay, không restart)
- `GET /api/settings` [O] → `AppSettings` · `PUT /api/settings` [H]
- `GET /api/onboarding` [O] · `POST /api/onboarding/complete` [H]
- `GET /api/events` [V] SSE `ServerEvent` (Viewer chỉ nhận `video`)
- `GET /api/audit?limit=` [O] → `AuditEvent[]`
- `POST /api/system/backup` [H] → tạo bản sao SQLite trong `<data>/backups/`

## Skills
- `GET /api/skills?q=&sourceId=` [O] → `Skill[]` · `GET /api/skills/:id` [O] → `SkillDetail`
- `GET /api/skill-sources` [O] → `SkillSource[]`
- `POST /api/skill-sources/folder` [H] `{ path, watch? }`
- `POST /api/skill-sources/github` [H] `{ url, ref?, private? }`
- `PATCH /api/skill-sources/:id` [H] `{ enabled?, watch?, pinned? }`
- `POST /api/skill-sources/:id/rescan` [H]
- `POST /api/skill-sources/:id/check-update` [H] → `UpdateCheck`
- `POST /api/skill-sources/:id/apply-update` [H] `{ sha }`
- `DELETE /api/skill-sources/:id` [H]
- `POST /api/skills/:id/install` [H] `{ provider, confirm: true, allowScripts: boolean }` · `DELETE /api/skills/:id/install/:provider` [H]
- `GET /api/github/status` [O] · `POST /api/github/device/start` [H] → `DeviceFlowStart` · `POST /api/github/device/poll` [H] · `POST /api/github/use-gh` [H] `{ confirm: true }` · `POST /api/github/logout` [H]

## AI CLI & cloudflared
- `GET /api/tools` [O] → `ToolStatus[]` · `POST /api/tools/:id/refresh` [H]
- `POST /api/tools/:id/install` [H] `{ methodId, confirm: true }` → `Job` (log theo `GET /api/jobs/:id`)
- `GET /api/tools/:id/diagnostics` [H] → `{ lines: string[] }`

## Web server & tunnel
- `GET /api/server/status` [O] → `ServerStatus`
- `GET /api/server/config` [O] · `PUT /api/server/config` [H] `ServerConfig`
- `POST /api/server/start|stop|restart` [H]
- `GET /api/server/qr?url=` [O] → `image/svg+xml`
- `GET /api/server/firewall-guide` [O] → `{ os, steps: string[], commands: string[] }`
- `GET /api/tunnel/status` [O] · `PUT /api/tunnel/config` [H] `TunnelConfig` · `POST /api/tunnel/start|stop` [H] · `DELETE /api/tunnel/token` [H]

## Video & jobs
- `GET /api/videos?kind=&q=` [V] (Viewer chỉ thấy `export` có `visibility=viewers`)
- `GET /api/videos/:id` [V] · `GET /api/videos/:id/thumbnail` [V]
- `GET /api/videos/:id/stream` [V] — hỗ trợ `Range` → `206 Partial Content`, `416` khi sai range
- `GET /api/videos/:id/download` [V] — `Content-Disposition: attachment`, stream
- `PATCH /api/videos/:id` [O] `{ title?, visibility? }` · `DELETE /api/videos/:id` [O] (gỡ khỏi thư viện; `?deleteFile=true` chỉ với export và yêu cầu [H])
- `POST /api/videos/scan` [H] — quét các thư mục nguồn đã cấp quyền
- `POST /api/videos/:id/share` [O] `{ ttlMinutes }` → `ShareLink` · `DELETE /api/shares/:id` [O] · `GET /s/:token` [P] (stream có hạn)
- `GET /api/jobs` [O] → `Job[]` · `GET /api/jobs/:id` [O] → `JobDetail`
- `POST /api/jobs/render` [O] `RenderRequest` → `Job`
- `POST /api/jobs/:id/cancel` [O] · `POST /api/jobs/:id/retry` [O]

Không có route nào trả đường dẫn đĩa thật cho Viewer; `id` là UUID ngẫu nhiên (chặn IDOR bằng kiểm tra quyền từng tài nguyên).

# Doto Skill — HTTP API (v0.2)

Kiểu dữ liệu: `apps/web/src/api/types.ts` (nguồn sự thật cho client) ↔ `crates/server/src/{routes,v02,memory,public}.rs` (serde `camelCase`).

## Mô hình listener & quyền

| Listener | Bind | Ai dùng | Quyền tối đa |
| --- | --- | --- | --- |
| **Control** (nội bộ) | `127.0.0.1:<cổng ngẫu nhiên>` luôn bật khi app chạy | Cửa sổ desktop (Tauri) | Owner + `hostControl` |
| **Public web** | Off / `127.0.0.1:<port>` / LAN `0.0.0.0` hoặc IP interface | Bản Web (trình duyệt PC/tablet/mobile, PWA, cloudflared) | Owner (đầy đủ chức năng) / Viewer |

- Bản App và bản Web có **đầy đủ chức năng ngang nhau cho Owner** (nhận diện phần cứng, probe/benchmark FFmpeg GPU, quản lý Skills & AI CLI theo allowlist cố định, cấu hình mạng/tunnel, sao lưu/khôi phục SQLite) — toàn bộ thực thi trên phần cứng máy chủ của bản App. Không có route nào nhận lệnh shell hay đường dẫn thực thi tuỳ ý; Viewer bị chặn (`403`/`404`) khỏi toàn bộ route `[O]`.
- Xác thực cùng miền (same-origin): cookie `doto_session` / `doto_ctl` (HttpOnly, SameSite=Strict, `Secure` khi qua HTTPS/`X-Forwarded-Proto: https`) kèm CSRF header `x-csrf-token`.
- Xác thực chéo miền (bản Web độc lập kết nối tới URL máy chủ bản App): gửi `authMode: "bearer"` lúc login/pair để nhận `sessionToken`, gọi API với `Authorization: Bearer <token>` (không dùng cookie chéo miền) và lấy vé ngắn hạn `POST /api/auth/ticket` (10 phút) cho `<video>`, `<img>`, `<a download>`, `EventSource`.
- Rate limit đăng nhập/ghép nối: 10 lần/5 phút/IP. IP allowlist (CIDR) áp dụng cho listener Public.
- Lỗi: `{ "error": "<code>", "message": "<tiếng Việt>" }` với 400/401/403/404/409/413/416/429/500.

Ký hiệu: `[P]` public không cần đăng nhập · `[V]` Viewer trở lên · `[O]` Owner · `[C]` chỉ listener Control / loopback.

## Auth & người dùng
- `GET  /api/health` [P] → `{ ok, version, uptimeSeconds, hostname }`
- `GET  /api/auth/me` [P] → `Me`
- `POST /api/auth/setup` [C, chỉ khi chưa đặt mật khẩu Owner] `{ username, password, authMode? }` → tạo Owner đầu tiên (mật khẩu ≥ 10 ký tự, có chữ & số)
- `POST /api/auth/desktop` [C] `{ token }` → phiên Owner cho cửa sổ Tauri
- `POST /api/auth/login` [P] `{ username, password, deviceName?, authMode? }` → `Me`
- `POST /api/auth/pair` [P] `{ code, deviceName, authMode? }` → phiên theo role của mã ghép nối
- `POST /api/auth/ticket` [V] → `{ ticket, expiresIn: 600 }` (chỉ dùng cho GET media/QR/SSE)
- `POST /api/auth/logout` [V]
- `GET/POST /api/users` [O] · `DELETE /api/users/:id` [O] · `POST /api/users/:id/password` [O]
- `GET /api/devices` [O] → `DeviceSession[]` · `DELETE /api/devices/:id` [O] (thu hồi)
- `POST /api/pairing` [O] `{ role, ttlMinutes }` → `PairingCode`

## Hệ thống & tài nguyên
- `GET /api/system/hardware` [O] → `HardwareInfo` · `POST /api/system/hardware/probe` [O]
- `GET /api/system/metrics` [O] → `Metrics`
- `GET /api/system/ffmpeg` [O] → `FfmpegStatus` · `POST /api/system/ffmpeg/probe` [O] (dò lại + encode thử) · `POST /api/system/ffmpeg/benchmark` [O] (benchmark 60 frame 720p)
- `GET /api/settings/resources` [O] → `ResourceSettings` · `PUT` [O] `{ profile, overrides }` (áp dụng ngay, không restart)
- `GET /api/settings` [O] → `AppSettings` · `PUT /api/settings` [O] (xác minh binary FFmpeg/ffprobe thật trước khi lưu)
- `GET /api/onboarding` [O] · `POST /api/onboarding/complete` [O]
- `GET /api/events` [V] SSE `ServerEvent` (Viewer chỉ nhận `video`)
- `GET /api/audit?limit=` [O] → `AuditEvent[]`
- `GET /api/system/backups` [O] → `BackupEntry[]` · `POST /api/system/backup` [O] · `POST /api/system/restore` [O] `{ file, confirm: true }`

## Skills
- `GET /api/skills?q=&sourceId=` [O] → `Skill[]` · `GET /api/skills/:id` [O] → `SkillDetail`
- `GET /api/skill-sources` [O] → `SkillSource[]`
- `POST /api/skill-sources/folder` [O] `{ path, watch? }`
- `POST /api/skill-sources/github` [O] `{ url, ref?, private? }`
- `POST /api/skill-sources/archive?filename=&overwrite=&autoMd=&confirm=true` [O] (raw binary ≤ 100 MB: `.zip`, `.tar.gz`, `.tgz`, `.tar`, `.7z`, `.rar` → `SkillImportReport`)
- `POST /api/skill-sources/upload-folder` [O] `{ folderName, files: [{ path, contentBase64 }], overwrite?, autoMd?, confirm: true }` → `SkillImportReport`
- `PATCH /api/skill-sources/:id` [O] `{ enabled?, watch?, pinned? }`
- `POST /api/skill-sources/:id/rescan` [O]
- `POST /api/skill-sources/:id/check-update` [O] → `UpdateCheck`
- `POST /api/skill-sources/:id/apply-update` [O] `{ sha }`
- `DELETE /api/skill-sources/:id` [O]
- `POST /api/skills/:id/install` [O] `{ provider, confirm: true, allowScripts: boolean }` · `DELETE /api/skills/:id/install/:provider` [O]
- `GET /api/github/status` [O] · `POST /api/github/device/start` [O] → `DeviceFlowStart` · `POST /api/github/device/poll` [O] · `POST /api/github/use-gh` [O] `{ confirm: true }` · `POST /api/github/logout` [O]

## AI CLI, AI Studio & cloudflared
- `GET /api/tools` [O] → `ToolStatus[]` · `POST /api/tools/:id/refresh` [O]
- `POST /api/tools/:id/install` [O] `{ methodId, confirm: true }` → `Job` (allowlist cố định theo OS, log theo `GET /api/jobs/:id`)
- `GET /api/tools/:id/diagnostics` [O] → `{ lines: string[] }`
- `GET /api/studio/catalog` [O] → `{ providers: StudioProviderCatalog[] }`
- `GET /api/studio/runs` [O] → `StudioRunDetail[]` · `GET /api/studio/runs/:id` [O] → `StudioRunDetail`
- `POST /api/studio/runs` [O] `StudioRunRequest` (`mode`: `single` | `multi_same_ai` | `multi_ai` | `custom_batch`, `confirm: true`) → `StudioRunDetail`
- `POST /api/studio/runs/:id/cancel` [O] · `POST /api/studio/sessions/:id/cancel` [O]

## Web server, Cloudflare Access & Multi-Tunnel CRUD
- `GET /api/server/status` [O] → `ServerStatus`
- `GET /api/server/config` [O] · `PUT /api/server/config` [O] `ServerConfig` (hỗ trợ `requireCfAccess`, `cfAccessTeamDomain`, `cfAccessAud`)
- `POST /api/server/start|stop|restart` [O]
- `GET /api/server/qr?url=` [O] → `image/svg+xml`
- `GET /api/server/firewall-guide` [O] → `{ os, steps: string[], commands: string[] }`
- `GET /api/tunnel/status` [O] · `PUT /api/tunnel/config` [O] `TunnelConfig` · `POST /api/tunnel/start|stop` [O] · `DELETE /api/tunnel/token` [O]
- `GET /api/tunnels` [O] → `TunnelsOverview` (`managed` + `external` cloudflared processes)
- `POST /api/tunnels` [O] `ManagedTunnelReq` → `ManagedTunnel` (token lưu OS Vault `cloudflare.tunnel.<id>`)
- `PATCH /api/tunnels/:id` [O] `ManagedTunnelReq` · `DELETE /api/tunnels/:id` [O]
- `POST /api/tunnels/:id/start|stop` [O] · `POST /api/tunnels/quick-web` [O] `{ confirm: true }`
- `POST /api/tunnels/processes/:pid/stop` [O] `{ confirm: true }` (chỉ dừng tiến trình xác minh tên `cloudflared`)

## Duyệt thư mục máy chủ, Mobile Media Uploads & Bộ nhớ chung
- `GET /api/fs/dirs?path=` [O] → `HostDirsResponse` (chỉ liệt kê thư mục con, không đọc nội dung tệp, bỏ symlink)
- `POST /api/fs/mkdir` [O] `{ parent, name, confirm: true }` → `{ ok, path }`
- `POST /api/videos/upload?filename=&title=` [O] (stream nhị phân ≤ 1 GB cho `video/*` và `image/*`; video tự động lập chỉ mục vào `video_assets`)
- `GET /api/uploads` [O] → `UploadedMedia[]` · `GET /api/uploads/:id/preview` [O] · `DELETE /api/uploads/:id` [O]
- `GET /api/memory` [O] → `MemoryOverview` · `GET /api/memory/doc?id=` [O] → `MemoryDocContent`
- `PUT /api/memory/doc` [O] `{ id, content, confirm: true }` (allowlist tệp Markdown cố định, tự lọc `[REDACTED]` bí mật)
- `POST /api/memory/append` [O] `{ id?, agent?, note }` · `POST /api/memory/init` [O] `{ overwrite?, confirm: true }` · `POST /api/memory/handoff` [O] `{ agent, summary, nextAction? }`

## Video & jobs
- `GET /api/videos?kind=&q=` [V] (Viewer chỉ thấy `export` có `visibility=viewers`)
- `GET /api/videos/:id` [V] · `GET /api/videos/:id/thumbnail` [V]
- `GET /api/videos/:id/stream` [V] — hỗ trợ `Range` → `206 Partial Content`, `416` khi sai range
- `GET /api/videos/:id/download` [V] — `Content-Disposition: attachment`, stream
- `PATCH /api/videos/:id` [O] `{ title?, visibility? }` · `DELETE /api/videos/:id` [O] (gỡ khỏi thư viện; `?deleteFile=true` chỉ với `export`)
- `POST /api/videos/scan` [O] — quét các thư mục nguồn đã cấp quyền
- `GET /api/videos/:id/shares` [O] · `POST /api/videos/:id/share` [O] `{ ttlMinutes }` → `ShareLink` · `DELETE /api/shares/:id` [O] · `GET /s/:token` [P] (stream có hạn)
- `GET /api/jobs` [O] → `Job[]` · `GET /api/jobs/:id` [O] → `JobDetail`
- `POST /api/jobs/render` [O] `RenderRequest` → `Job`
- `POST /api/jobs/:id/cancel` [O] · `POST /api/jobs/:id/retry` [O]

Không có route nào trả đường dẫn đĩa thật cho Viewer; `id` là UUID ngẫu nhiên (chặn IDOR bằng kiểm tra quyền từng tài nguyên).

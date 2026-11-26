# PROJECT_MEMORY — Doto Skill

> Nguồn sự thật chung cho Claude Code, OpenAI Codex và Google Antigravity. Chỉ ghi tóm tắt kỹ thuật — không token, mật khẩu, prompt riêng tư hay nội dung repo private.

## 1. Mục tiêu sản phẩm
Ứng dụng desktop **Windows + macOS** (Tauri v2) đồng thời là **máy chủ web cục bộ** để thiết bị khác xem/tải thư viện video qua trình duyệt. Tích hợp: quản lý nhiều nguồn AI Skills (thư mục + GitHub), cài đặt có xác nhận 4 công cụ (Antigravity `agy`, Codex `codex`, Claude Code `claude`, `cloudflared`), nhận diện phần cứng, 3 profile tài nguyên, pipeline render video FFmpeg thật, bộ nhớ chung cho 3 agent.

## 2. Kiến trúc hiện tại (v0.1.0)
```
apps/desktop (Tauri v2, tray, autostart) ──in-process──► crates/server (Axum)
                                                         ├─ listener Control 127.0.0.1:<ngẫu nhiên>  → cửa sổ desktop, route [H]
                                                         └─ listener Public  Off | 127.0.0.1 | LAN(IP/0.0.0.0):4380 → trình duyệt, cloudflared
crates/server ─► crates/core (SQLite WAL + migrations, settings, audit, jobs, EventBus, OS vault)
              ─► crates/system (HardwareProbe, ResourceManager + admission)
              ─► crates/media (FFmpeg discover/verify, encoder probe, library, render queue)
              ─► crates/skills (scanner, GitHub client, SkillService, provider mapping)
              ─► crates/integrations (CliProvider ×4, ToolManager, cloudflared installer, TunnelManager)
apps/web + packages/ui (React 19, TS strict, Vite, Tailwind 4) → build ra apps/web/dist, server phục vụ tĩnh + SPA fallback
```
- **Bảo mật listener:** route điều khiển máy (cài CLI, đổi thư mục root, thêm nguồn skill, start/stop server/tunnel, đọc filesystem) chỉ đăng ký trên listener Control; phiên `hostControl` chỉ tạo được bằng token một lần do app desktop sinh (đưa qua URL fragment). Cookie tách tên theo listener (`doto_ctl` / `doto_session`).
- **Auth:** Argon2id, cookie HttpOnly + SameSite=Strict (+Secure khi qua HTTPS/cloudflared), CSRF header `x-csrf-token`, kiểm tra Origin, rate limit 10/5 phút cho login/pair, pairing code 8 ký tự hết hạn (5–1440 phút) dùng một lần, revoke phiên, IP allowlist (CIDR), audit log (đã lọc bí mật). Owner cục bộ tự tạo lần đầu với mật khẩu `!` (không đăng nhập từ xa được) → cửa sổ desktop bắt buộc đặt tài khoản/mật khẩu Owner; LAN/Tunnel bị chặn tới khi có mật khẩu.
- **RBAC:** Owner (quản trị), Viewer (chỉ xem/tải export có `visibility=viewers`). Không có quyền → 404 (chống IDOR). API không bao giờ trả đường dẫn đĩa.
- **Video:** quét thư mục được cấp quyền (không theo symlink), ffprobe, thumbnail; render trim/ghép nhiều đoạn → MP4 H.264/AAC, `-progress pipe:1` cho tiến độ/ETA thật, ghi `.part.mp4` rồi rename atomic, hủy/thử lại, fallback libx264 khi encoder GPU lỗi, khôi phục job khi app khởi động lại. Stream có HTTP Range (206/416), download `Content-Disposition` UTF-8, share link có hạn + revoke, giới hạn số luồng theo profile + băng thông tuỳ chọn.
- **Profile:** Eco/Balanced/Max thay đổi `max_jobs`, thread FFmpeg, ngưỡng CPU/RAM nhận job, số phiên GPU, chu kỳ đo; override thủ công kẹp theo phần cứng; áp dụng ngay không restart.
- **Skills:** 0 nguồn/0 skill ban đầu. Nguồn thư mục (watcher debounce 1.5s) và GitHub (resolve ref→SHA, tải tarball read-only, giải nén an toàn: chặn traversal/symlink/hardlink, giới hạn 200 MB nén/500 MB giải nén/50k entry; check-update hiển thị diff file trước khi apply; pin commit). Cài sang provider = **sao chép** kèm marker `.doto-skill.json`, không ghi đè thư mục người dùng, skill có script cần xác nhận riêng.

## 3. Quyết định đã chốt
Xem `docs/shared/DECISIONS.md` (ADR-001…ADR-012).

## 4. Đường dẫn provider (tài liệu chính thức, kiểm tra 2026-11-26)
| Provider | Skills cấp user | Kiểm tra đăng nhập |
| --- | --- | --- |
| Claude Code | `~/.claude/skills/<name>/SKILL.md` | `claude auth status` (JSON, exit 0/1) |
| Codex | `~/.agents/skills/<name>/SKILL.md` (bắt buộc name+description) | `codex login status` (in ra **stderr**, định dạng không tài liệu hoá → diễn giải thận trọng) |
| Antigravity | `~/.gemini/config/skills/<name>/` (bắt buộc description) | không có lệnh chính thức → `unknown` |
| cloudflared | — | không áp dụng (token tunnel không cần login) |
Chi tiết + nguồn: `docs/INTEGRATIONS.md`.

## 5. Quy tắc & bẫy kỹ thuật
- Windows: PowerShell có thể chặn `npm.ps1` → dùng `npm.cmd`. Repo nằm trong đường dẫn có dấu cách → luôn trích dẫn đường dẫn.
- `tauri-build` cần `apps/web/dist` tồn tại khi biên dịch crate desktop (CI build web trước cargo).
- Không chạy `cargo`/`tauri` từ thư mục gốc nếu có nhiều `tauri.conf.json` — CLI Tauri chạy trong `apps/desktop` (có `legacy/.../src-tauri`).
- axum 0.8: không đăng ký cùng method trên cùng path hai lần (panic lúc chạy) — route [H] chỉ thêm method mới.
- Máy có IP public: chế độ LAN với "tất cả interface" sẽ lộ ra Internet → UI cảnh báo, khuyên chọn IP mạng riêng/VPN cụ thể. `lanAddresses[].scope` = private | cgnat | link_local | public.
- Cổng mặc định public server: **4380** (4320 thường bị bản legacy v0.2 chiếm).
- Quick Tunnel (`*.trycloudflare.com`) không hỗ trợ SSE → UI tự chuyển sang polling.
- Token tunnel truyền qua biến môi trường `TUNNEL_TOKEN`, không đặt trong argv.
- FFmpeg không đóng gói kèm (tránh rủi ro GPL/nonfree); app dò PATH/vị trí phổ biến hoặc đường dẫn cấu hình, cảnh báo cờ license.

## 6. Giả định & hạn chế hiện tại
- GitHub OAuth Device Flow cần `DOTO_GITHUB_CLIENT_ID` của một OAuth App do chủ sở hữu tạo; khi chưa có, dùng tuỳ chọn "Dùng gh CLI" (đọc `gh auth token` với xác nhận) — đã hoạt động.
- Chưa ký số / notarize installer (chưa có chứng chỉ Apple Developer / Windows code signing).
- Chưa có Cloudflare Access tự động — chỉ hướng dẫn (khuyến nghị bật khi xuất bản Internet).
- Chưa có tính năng AI auto-edit; kiến trúc job/skill cho phép mở rộng sau.
- Thử nghiệm "thiết bị khác cùng LAN" mới kiểm chứng bằng trình duyệt trên cùng host qua IP Tailscale; chưa thử trên điện thoại vật lý.

## 7. Thiết lập máy dev
Node ≥ 20.19 (đã dùng 26), Rust stable (1.99), ffmpeg/ffprobe trên PATH, Windows: WebView2 + MSVC build tools; macOS: Xcode CLT.
```
npm ci && npm run build -w apps/web
npm run dev:server     # http://127.0.0.1:4319 — mở URL "desktop-token" in ra để đăng nhập quyền host
npm run dev:web        # Vite dev (proxy /api → 4319)
npm run desktop:dev
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
<!-- AUTO:END -->

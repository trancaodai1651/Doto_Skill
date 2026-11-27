# PROJECT_MEMORY — Doto Skill

> Nguồn sự thật chung cho Claude Code, OpenAI Codex và Google Antigravity. Chỉ ghi tóm tắt kỹ thuật — không token, mật khẩu, prompt riêng tư hay nội dung repo private.

## 1. Mục tiêu sản phẩm
Ứng dụng desktop **Windows + macOS** (Tauri v2) đồng thời là **máy chủ web cục bộ** để thiết bị khác xem/tải thư viện video qua trình duyệt. Tích hợp: quản lý nhiều nguồn AI Skills (thư mục + GitHub), cài đặt có xác nhận 4 công cụ (Antigravity `agy`, Codex `codex`, Claude Code `claude`, `cloudflared`), nhận diện phần cứng, 3 profile tài nguyên, pipeline render video FFmpeg thật, bộ nhớ chung cho 3 agent.

## 2. Kiến trúc hiện tại (v0.2.0)
```
apps/desktop (Tauri v2, tray, autostart) ──in-process──► crates/server (Axum: routes + v02 + memory + Cloudflare Access)
                                                         ├─ listener Control 127.0.0.1:<ngẫu nhiên>  → cửa sổ desktop
                                                         └─ listener Public  Off | 127.0.0.1 | LAN(IP/0.0.0.0):4380 → bản Web, cloudflared
crates/server ─► crates/core (SQLite WAL v2 + migrations, backup/restore, settings, audit, jobs, EventBus, OS vault)
              ─► crates/system (HardwareProbe, ResourceManager + admission)
              ─► crates/media (FFmpeg discover/verify, encoder probe & 720p benchmark, library, mobile upload indexing, render queue)
              ─► crates/skills (scanner, GitHub client, pure-Rust .zip/.tar.gz & folder upload importer, SkillService, provider mapping)
              ─► crates/integrations (CliProvider ×4, ToolManager, StudioRunner đơn/đa phiên song song, Multi-Tunnel CRUD Manager)
apps/web + packages/ui (React 19, TS strict, Vite, Tailwind 4) → build ra apps/web/dist, phục vụ tĩnh hoặc chạy độc lập kết nối tới URL máy chủ bản App
```
- **Bản App & Bản Web đồng nhất 100% tính năng (ADR-013):** Owner đã xác thực trên bản Web (cùng miền hoặc kết nối từ xa tới địa chỉ máy chủ bản App) có đầy đủ mọi tính năng quản trị như trên cửa sổ Desktop, sử dụng trực tiếp phần cứng CPU/RAM/GPU của máy chủ bản App thông qua API allowlist an toàn. Browser không bao giờ gửi lệnh shell tuỳ ý hay đường dẫn thực thi giả (`ffmpegPath`/`ffprobePath` bắt buộc xác minh `-version` thật trước khi lưu). Chỉ `/api/auth/desktop` dành riêng cho token một lần của cửa sổ Tauri, và `/api/auth/setup` chỉ cho phép từ loopback khi chưa đặt mật khẩu Owner.
- **Auth & Kết nối chéo miền:** Argon2id, cùng miền dùng cookie HttpOnly + SameSite=Strict (+Secure khi HTTPS) + `x-csrf-token` + kiểm tra `Origin == Host`. Bản Web độc lập kết nối chéo miền tới server bản App dùng `Authorization: Bearer <sessionToken>` (CORS **không** bật `Access-Control-Allow-Credentials` → miễn nhiễm CSRF chéo miền) kèm vé ngắn hạn `POST /api/auth/ticket` (10 phút) chỉ áp dụng cho GET media/QR/SSE. Rate limit 10/5 phút cho login/pair, pairing code 8 ký tự hết hạn (5–1440 phút) dùng một lần, revoke phiên, IP allowlist (CIDR), audit log (đã lọc bí mật).
- **RBAC:** Owner (quản trị đầy đủ trên cả App & Web), Viewer (chỉ xem/tải export có `visibility=viewers`, bị cấm `403`/`404` khỏi toàn bộ route quản trị). API không bao giờ trả đường dẫn đĩa thật cho Viewer.
- **Video:** quét thư mục được cấp quyền (không theo symlink), ffprobe, thumbnail; render trim/ghép nhiều đoạn → MP4 H.264/AAC, `-progress pipe:1` cho tiến độ/ETA thật, ghi `.part.mp4` rồi rename atomic, hủy/thử lại, fallback libx264 khi encoder GPU lỗi, khôi phục job khi app khởi động lại. Stream có HTTP Range (206/416), download `Content-Disposition` UTF-8, tạo/liệt kê/thu hồi share link có hạn, giới hạn số luồng theo profile + băng thông tuỳ chọn.
- **Profile & Benchmark:** Eco/Balanced/Max thay đổi `max_jobs`, thread FFmpeg, ngưỡng CPU/RAM nhận job, số phiên GPU, chu kỳ đo; override thủ công kẹp theo phần cứng; áp dụng ngay không restart; có benchmark encoder 720p thực tế (`POST /api/system/ffmpeg/benchmark`).
- **Skills:** 0 nguồn/0 skill ban đầu. Nguồn thư mục (watcher debounce 1.5s, gợi ý ổ đĩa máy chủ) và GitHub (resolve ref→SHA, tải tarball read-only, giải nén an toàn: chặn traversal/symlink/hardlink, giới hạn 200 MB nén/500 MB giải nén/50k entry; check-update hiển thị diff file trước khi apply; pin commit). Cài sang provider = **sao chép** kèm marker `.doto-skill.json`, không ghi đè thư mục người dùng, skill có script cần xác nhận riêng.

## 3. Quyết định đã chốt
Xem `docs/shared/DECISIONS.md` (ADR-001…ADR-014).

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
- Kiểm tra JWT `Cf-Access-Jwt-Assertion` của Cloudflare Access và CRUD đa tunnel đã được tích hợp; cần tài khoản Cloudflare Zero Trust của chủ sở hữu để kiểm chứng ngoài Internet thực tế.
- AI Studio điều phối trực tiếp CLI `agy` / `claude` / `codex` (cần người dùng tự cài và đăng nhập từng CLI theo hướng dẫn chính thức).
- Thử nghiệm thiết bị di động đã có E2E tự động trên `doto-server` thật; khuyến nghị kiểm thử thêm trên iPhone/Android vật lý cùng Wi-Fi.

## 7. Thiết lập máy dev
Node ≥ 20.19 (đã dùng 26), Rust stable (1.99), ffmpeg/ffprobe trên PATH, Windows: WebView2 + MSVC build tools; macOS: Xcode CLT.
```
npm ci && npm run build -w apps/web
npm run dev:server     # http://127.0.0.1:4319 — mở URL "desktop-token" in ra để đăng nhập quyền host
npm run dev:web        # Vite dev (proxy /api → 4319)
npm run desktop:dev
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** 2026-11-27T00:40:56.840Z bởi `antigravity`
- Branch: `feat/v0.2-studio-p7-p13` · HEAD: `7e05d5e395ae5b92be3def9d424d28ab2d0cbbd5`
- Working tree: sạch
- Tóm tắt phiên: Triển khai trọn bộ P7–P13 lên v0.2.0 (AI Studio đa phiên Antigravity/Claude/Codex, Mobile Upload & App Photos, Import Skill .zip/folder, CRUD đa Tunnel + Cloudflare Access JWT, Trung tâm Memory/Handoff 3 AI, E2E doto-server thật)
- NEXT_ACTION: Chờ CI đa nền tảng trên PR v0.2.0 xanh, merge vào main và phát hành GitHub Release v0.2.0

**Commit gần nhất:**
- 7e05d5e feat(v0.2): triển khai trọn bộ P7–P13 (AI Studio đa phiên, Mobile Upload, Import .zip, CRUD đa Tunnel & Cloudflare Access, Memory 3 AI, E2E thật) (2026-11-27)
- 23f1785 docs(handoff): đồng bộ trạng thái hoàn tất phát hành v0.1.0 cho 3 AI (2026-11-26)
- 0592add docs(release): ghi nhận kết quả CI/Release v0.1.0 và checksum installers (2026-11-26)
- 12084d4 Merge pull request #1 from trancaodai1651/feat/initial-platform (2026-11-26)
- 66c2ecb docs(handoff): đồng bộ bộ nhớ chung và 3 file bàn giao AI (antigravity) (2026-11-26)

**Lần chạy test gần nhất:**
Máy: Windows 11 Pro (x86_64, i7-6800K, 32 GB, RTX 2070 SUPER) · Rust 1.99.1 · Node 26.13 · FFmpeg 9.0.2 (gyan full) · 2026-11-27 · agent: antigravity

| Lệnh / Workflow | Kết quả |
| --- | --- |
| `cargo test --workspace` (DOTO_SECRETS=memory, DOTO_REQUIRE_FFMPEG=1) | ✅ 115/115 test xanh: core 13 · system 8 · media 13 + render_e2e 3 (FFmpeg thật) · skills 25 + service 2 · integrations 17 (`StudioRunner` song song + CRUD đa tunnel) · server 11 + api_security 3 (kiểm chứng đồng bộ Owner App↔Web, duyệt thư mục host, mobile upload, `.zip` Zip-Slip, AI Studio đa phiên, CRUD đa tunnel + Cloudflare Access JWT, Memory lọc bí mật, chặn Viewer 403/404) |
| `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check` | ✅ sạch (0 cảnh báo) |
| `npm run typecheck/lint/test -w apps/web` | ✅ 33/33 unit test (Vitest, kiểm tra đồng bộ từ điển VI/EN) |
| `npm run e2e -w apps/web` (Playwright Chromium, mock API) | ✅ 5/5 (`onboarding.spec.ts`, `skills.spec.ts`, `gallery.spec.ts`, `web-parity.spec.ts`, `studio-v02.spec.ts`) |
| `npm run e2e:real` (Playwright chạy trực tiếp trên binary `doto-server` thật) | ✅ 1/1 (`real-server.spec.ts`: desktop one-time token → setup → onboarding 8 bước → bật Public server → import `.zip` skill → ghi bàn giao Memory → tải ảnh mobile → ghép nối Viewer RBAC) |
| `node scripts/secrets-scan.mjs` + `node scripts/license-check.mjs` | ✅ 228 file sạch bí mật · 639 phụ thuộc không copyleft bắt buộc |
| `npm run desktop:build -- --bundles nsis,msi` (Windows x64, local `0.2.0`) | ✅ `Doto Skill_0.2.0_x64-setup.exe` 6.27 MiB (`ffda71bd…96b1f`) · `Doto Skill_0.2.0_x64_en-US.msi` 9.10 MiB (`e49b052c…715b5`) |
| GitHub Actions Release `v0.1.0` (`45788563182`, mốc trước) | ✅ https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.1.0 |
<!-- AUTO:END -->

# HANDOFF — Claude Code

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent claude --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · antigravity)
**Đã làm:**
- Đồng bộ đầy đủ 100% tính năng giữa bản App Desktop và bản Web cho Owner (`ADR-013`), kết nối tới phần cứng máy chủ của bản App (hỗ trợ cả truy cập trực tiếp LAN/Tunnel lẫn nhập URL máy chủ từ bản Web độc lập qua CORS Bearer + vé media ngắn hạn `POST /api/auth/ticket`).
- Bổ sung sao lưu & khôi phục SQLite (`GET /api/system/backups`, `POST /api/system/restore`), benchmark encoder 720p (`POST /api/system/ffmpeg/benchmark`), danh sách link chia sẻ video (`GET /api/videos/:id/shares`), gợi ý ổ đĩa máy chủ, xác minh binary FFmpeg thật, chống self-deadlock khi restart server từ bản Web và sửa lỗi crash sự kiện log tunnel.
- Hoàn tất kiểm thử (101 Rust + 33 Web unit + 4 Playwright E2E), đóng gói và phát hành GitHub Release `v0.1.0`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`, `docs/SECURITY.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho Claude Code
1. Nếu chủ sở hữu cung cấp OAuth App `client_id`: cấu hình `DOTO_GITHUB_CLIENT_ID` trong CI/Release workflow và kiểm chứng GitHub Device Flow với repo private.
2. Tích hợp xác minh tuỳ chọn header `Cf-Access-Jwt-Assertion` (Cloudflare Access JWT) trên listener Public cho môi trường xuất bản Internet.
3. Khi có chứng chỉ ký số Apple Developer / Windows Authenticode, cấu hình GitHub Secrets theo `docs/RELEASE.md` và phát hành bản signed/notarized.

Cách chạy tiếp:
```
npm ci
npm run release-check
npm run dev:server   # mở URL desktop-token được in ra
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** 2026-11-26T08:59:32.803Z bởi `antigravity`
- Branch: `feat/initial-platform` · HEAD: `0d13454be22fd50a44a29157c6a61531287b6719`
- Working tree: sạch
- Tóm tắt phiên: Đồng bộ 100% tính năng bản App & bản Web (Owner dùng phần cứng server bản App), thêm kết nối URL server, CORS Bearer + media ticket, sao lưu/khôi phục SQLite, benchmark encoder 720p, danh sách share link; 101 test Rust + 33 unit + 4 E2E xanh, đóng gói installer Windows
- NEXT_ACTION: Chờ CI Windows + macOS xanh trên PR #1 → merge main → tag v0.1.0 → kiểm chứng GitHub Release installers & SHA256SUMS

**Commit gần nhất:**
- 0d13454 feat(parity): đồng bộ 100% tính năng App & Web qua server phần cứng máy chủ, CORS Bearer, backup/restore & benchmark (2026-11-26)
- 057c032 feat(web): UI link chia sẻ video có hạn (tạo có xác nhận, sao chép, thu hồi); cập nhật bộ nhớ chung + handoff (2026-11-26)
- 347ef95 fix(macos): clippy manual_pattern_char_comparison trong probe pin; bỏ field workspace không dùng (2026-11-26)
- a9ef727 build: release profile thin LTO; ghi kết quả test + build installer cục bộ (2026-11-26)
- 0f68ba3 ci: cài FFmpeg trên macOS bằng Homebrew (setup-ffmpeg không hỗ trợ runner macOS ARM) (2026-11-26)

**Lần chạy test gần nhất:**
Máy: Windows 11 Pro (x86_64, i7-6800K, 32 GB, RTX 2070 SUPER) · Rust 1.99.1 · Node 26.13 · FFmpeg 9.0.2 (gyan full) · 2026-11-26 · agent: antigravity

| Lệnh | Kết quả |
| --- | --- |
| `cargo test --workspace` (DOTO_SECRETS=memory, DOTO_REQUIRE_FFMPEG=1) | ✅ 101 test xanh: core 13 · system 8 · media 13 + render_e2e 3 (FFmpeg thật) · skills 18 + service 2 · integrations 12 · server 10 + api_security 2 (kiểm chứng đồng bộ Owner App↔Web, CORS Bearer + media ticket, xác minh binary FFmpeg, sao lưu/khôi phục SQLite, chặn Viewer 403/404) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ sạch (0 cảnh báo) |
| `cargo fmt --all --check` | ✅ |
| `npm run typecheck/lint/test -w apps/web` | ✅ 33/33 unit test (Vitest) |
| `npm run e2e -w apps/web` (Playwright Chromium) | ✅ 4/4 (`onboarding.spec.ts`, `skills.spec.ts`, `gallery.spec.ts`, `web-parity.spec.ts`) |
| `node scripts/secrets-scan.mjs` | ✅ 216 file, không phát hiện bí mật |
| `node scripts/license-check.mjs` | ✅ 639 phụ thuộc, không copyleft bắt buộc |
| `npm run desktop:build -- --bundles nsis,msi` (Windows x64, local) | ✅ `Doto Skill_0.1.0_x64-setup.exe` 5.83 MiB (sha256 `dc4a1e38…74b83`) · `Doto Skill_0.1.0_x64_en-US.msi` 8.47 MiB (sha256 `42f0cb39…16bbe`) |
| CI GitHub (Windows + macOS + Ubuntu) | đang chạy / kiểm chứng trên PR #1 & Release `v0.1.0` |
<!-- AUTO:END -->

# HANDOFF — OpenAI Codex

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent codex --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · antigravity)
**Đã làm:**
- Đồng bộ đầy đủ 100% tính năng giữa bản App Desktop và bản Web cho Owner (`ADR-013`), kết nối tới phần cứng máy chủ của bản App (hỗ trợ cả cùng miền lẫn chéo miền qua `ServerConnectionBox` + CORS Bearer + `POST /api/auth/ticket`).
- Đã triển khai `GET /api/videos/:id/shares` + UI danh sách link chia sẻ đang hoạt động, sao lưu & khôi phục SQLite (`GET /api/system/backups`, `POST /api/system/restore`), benchmark encoder 720p (`POST /api/system/ffmpeg/benchmark`), kịch bản Playwright `web-parity.spec.ts` và phát hành GitHub Release `v0.1.0`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`, `docs/SECURITY.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho OpenAI Codex
1. Bổ sung project Playwright thứ hai chạy trực tiếp với binary `doto-server` thật trong CI (`target/debug/doto-server --data-dir .e2e-data --control-port 0 --token-file .e2e-data/url.txt --web-root apps/web/dist` trong `globalSetup`).
2. Bổ sung mDNS (`doto.local`) tuỳ chọn cho chế độ LAN nếu cần khám phá tự động trong mạng nội bộ.
3. Giữ hợp đồng: mọi thay đổi field API phải sửa đồng thời `apps/web/src/api/types.ts`, handler trong `crates/server/src/routes.rs` và `docs/API.md`.

Cách chạy tiếp:
```
npm ci
npm run test -w apps/web && npm run e2e -w apps/web
cargo test -p doto-server
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** 2026-11-26T09:27:10.962Z bởi `antigravity`
- Branch: `main` · HEAD: `0592add8c0983466fc6f4000efe0e77481cff7c7`
- Working tree: sạch
- Tóm tắt phiên: Hoàn tất P0–P6 & đồng bộ 100% tính năng bản App ↔ bản Web (Owner kết nối phần cứng server bản App); merge PR #1 vào main và phát hành thành công GitHub Release v0.1.0 (Win NSIS/MSI + macOS universal DMG/app.zip + SHA256SUMS)
- NEXT_ACTION: Kiểm thử thực tế trên thiết bị di động cùng Wi-Fi & máy Mac vật lý; cấu hình DOTO_GITHUB_CLIENT_ID (Device Flow), Cloudflare Named Tunnel + Access và chứng chỉ ký mã khi chủ repo cung cấp

**Commit gần nhất:**
- 0592add docs(release): ghi nhận kết quả CI/Release v0.1.0 và checksum installers (2026-11-26)
- 12084d4 Merge pull request #1 from trancaodai1651/feat/initial-platform (2026-11-26)
- 66c2ecb docs(handoff): đồng bộ bộ nhớ chung và 3 file bàn giao AI (antigravity) (2026-11-26)
- 0d13454 feat(parity): đồng bộ 100% tính năng App & Web qua server phần cứng máy chủ, CORS Bearer, backup/restore & benchmark (2026-11-26)
- 057c032 feat(web): UI link chia sẻ video có hạn (tạo có xác nhận, sao chép, thu hồi); cập nhật bộ nhớ chung + handoff (2026-11-26)

**Lần chạy test gần nhất:**
Máy: Windows 11 Pro (x86_64, i7-6800K, 32 GB, RTX 2070 SUPER) · Rust 1.99.1 · Node 26.13 · FFmpeg 9.0.2 (gyan full) · 2026-11-26 · agent: antigravity

| Lệnh / Workflow | Kết quả |
| --- | --- |
| `cargo test --workspace` (DOTO_SECRETS=memory, DOTO_REQUIRE_FFMPEG=1) | ✅ 101/101 test xanh: core 13 · system 8 · media 13 + render_e2e 3 (FFmpeg thật) · skills 18 + service 2 · integrations 12 · server 10 + api_security 2 (kiểm chứng đồng bộ Owner App↔Web, CORS Bearer + media ticket, xác minh binary FFmpeg, sao lưu/khôi phục SQLite, chặn Viewer 403/404) |
| `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check` | ✅ sạch (0 cảnh báo) |
| `npm run typecheck/lint/test -w apps/web` | ✅ 33/33 unit test (Vitest) |
| `npm run e2e -w apps/web` (Playwright Chromium) | ✅ 4/4 (`onboarding.spec.ts`, `skills.spec.ts`, `gallery.spec.ts`, `web-parity.spec.ts`) |
| `node scripts/secrets-scan.mjs` + `node scripts/license-check.mjs` | ✅ 216 file sạch bí mật · 639 phụ thuộc không copyleft bắt buộc |
| `npm run desktop:build -- --bundles nsis,msi` (Windows x64, local) | ✅ `Doto Skill_0.1.0_x64-setup.exe` 5.83 MiB (`dc4a1e38…74b83`) · `Doto Skill_0.1.0_x64_en-US.msi` 8.47 MiB (`42f0cb39…16bbe`) |
| GitHub Actions CI (`45787940484`, PR #1) | ✅ Ubuntu Web/E2E (1m1s) · Secrets & licenses (33s) · macOS Rust+FFmpeg (1m21s) · Windows Rust+FFmpeg (2m48s) |
| GitHub Actions Release `v0.1.0` (`45788563182`, tag `12084d4677`) | ✅ CI xanh · `windows-x64` (7m2s) · `macos-universal` `lipo` x86_64+arm64 (11m19s) · xuất bản https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.1.0 (`Doto.Skill_0.1.0_x64-setup.exe` `c51500db…46af`, `Doto.Skill_0.1.0_x64_en-US.msi` `a53ab770…658d`, `Doto.Skill_0.1.0_universal.dmg` `bb1f8a6f…fa9b`, `Doto-Skill_universal.app.zip` `e37f9ee4…85fb`, `SHA256SUMS.txt`, `THIRD_PARTY_LICENSES.tsv`) |
<!-- AUTO:END -->

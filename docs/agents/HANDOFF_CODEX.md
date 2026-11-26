# HANDOFF — OpenAI Codex

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent codex --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · claude)
**Đã làm:** Khởi tạo Doto Skill v0.1.0 trên nền Rust/Tauri/React (P0–P4 đạt; P5/P6 phần lớn). Bản Node v0.2 cũ được giữ tại `legacy/studio-v0.2/`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho OpenAI Codex
1. (Đã xong bởi claude) UI share link — có thể thêm danh sách link đang hoạt động (cần API `GET /api/videos/:id/shares`, chưa có).
2. Playwright E2E chạy với server Rust thật: khởi động `target/debug/doto-server --data-dir .e2e-data --control-port 0 --token-file .e2e-data/url.txt --web-root apps/web/dist` trong `globalSetup`, dùng URL token để đăng nhập desktop.
3. Giữ hợp đồng: mọi thay đổi field API phải sửa đồng thời `apps/web/src/api/types.ts`, handler trong `crates/server/src/routes.rs` và `docs/API.md`.

Cách chạy tiếp:
```
npm ci
npm run test -w apps/web && npm run e2e -w apps/web
cargo test -p doto-server
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** 2026-11-26T07:34:19.040Z bởi `claude`
- Branch: `feat/initial-platform` · HEAD: `347ef959a1938b2808fe7f41d9542655c319f1a0`
- Working tree: sạch
- Tóm tắt phiên: Nền tảng v0.1.0: 6 crate Rust + Tauri + web; 101 test Rust + 32 unit + 3 e2e web xanh cục bộ; installer Windows build cục bộ; PR #1 đang chạy CI
- NEXT_ACTION: Chờ CI xanh trên Windows + macOS → merge PR #1 → tag v0.1.0 → kiểm tra Release workflow

**Commit gần nhất:**
- 347ef95 fix(macos): clippy manual_pattern_char_comparison trong probe pin; bỏ field workspace không dùng (2026-11-26)
- a9ef727 build: release profile thin LTO; ghi kết quả test + build installer cục bộ (2026-11-26)
- 0f68ba3 ci: cài FFmpeg trên macOS bằng Homebrew (setup-ffmpeg không hỗ trợ runner macOS ARM) (2026-11-26)
- 0b17206 feat: nền tảng Doto Skill v0.1.0 (Rust/Tauri/Axum/React) (2026-11-26)
- 9194145 chore: khởi tạo repository Doto Skill (2026-11-26)

**Lần chạy test gần nhất:**
Máy: Windows 11 Pro (x86_64, i7-6800K, 32 GB, RTX 2070 SUPER) · Rust 1.99.1 · Node 26.13 · FFmpeg 9.0.2 (gyan full) · 2026-11-26 · agent: claude

| Lệnh | Kết quả |
| --- | --- |
| `cargo test --workspace` (DOTO_SECRETS=memory) | ✅ core 13 · system 8 · media 13 + render_e2e 3 (FFmpeg thật) · skills 18 + service 2 (+1 mạng: import `anthropics/skills` thật, chạy với `--include-ignored`) · integrations 12 · server 10 + api_security 2 |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ sạch |
| `cargo fmt --all --check` | ✅ |
| `npm run typecheck/lint/test -w apps/web` | ✅ 32 unit test |
| `npm run e2e -w apps/web` (Playwright Chromium, API mock) | ✅ 3/3 |
| `node scripts/secrets-scan.mjs` | ✅ 214 file |
| `node scripts/license-check.mjs` | ✅ 639 phụ thuộc, không copyleft bắt buộc |
| Demo thủ công (browser pane + `doto-server` thật) | ✅ onboarding 8 bước, import GitHub 19 skill, LAN trên IP Tailscale 100.66.7.40:4380, scan, render h264_nvenc, Viewer qua URL LAN: stream 206, seek, download 200 (MP4 `ftyp`), /api/audit 403 |
| `npx tauri build --bundles nsis,msi` (Windows, local) | ✅ `Doto Skill_0.1.0_x64-setup.exe` 5.51 MiB (sha256 18615dc8…2607) · `Doto Skill_0.1.0_x64_en-US.msi` 7.82 MiB (sha256 2b09d2d3…1aeb); smoke test `doto-skill.exe`: /api/health 200, UI 200, API chưa đăng nhập 401 |
| CI GitHub (PR #1) | xem trạng thái PR |
<!-- AUTO:END -->

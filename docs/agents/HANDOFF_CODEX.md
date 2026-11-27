# HANDOFF — OpenAI Codex

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent codex --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-27 · antigravity)
**Đã làm:**
- Hoàn tất **P7 → P13 (`v0.2.0`)** đồng bộ 100% giữa bản App và bản Web (`ADR-014`): duyệt thư mục máy chủ, tải ảnh/video từ điện thoại & lưu về App Photos, import skill `.zip`/`.tar.gz` & tải thư mục trình duyệt, **AI Studio** đơn/đa phiên song song (`agy`/`claude`/`codex`), **CRUD đa Cloudflare Tunnel** + **Cloudflare Access JWT**, trung tâm **Shared Memory & Handoff 3 AI**, và đã bổ sung `npm run e2e:real` chạy Playwright trực tiếp trên binary `doto-server` thật trong CI!

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`, `docs/SECURITY.md`.

**Tests (máy dev Windows 11, 2026-11-27):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel/Access trên domain thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho OpenAI Codex
1. Mở rộng thêm kịch bản trong `apps/web/e2e-real/real-server.spec.ts` (ví dụ stream Range video thật bằng FFmpeg trong CI nếu cài thêm `ffmpeg` vào job Ubuntu).
2. Bổ sung mDNS (`doto.local`) tuỳ chọn cho chế độ LAN nếu cần khám phá tự động trong mạng nội bộ.
3. Giữ hợp đồng: mọi thay đổi field API phải sửa đồng thời `apps/web/src/api/types.ts`, handler trong `crates/server/src/{routes,v02}.rs` và `docs/API.md`.

Cách chạy tiếp:
```
npm ci
npm run test -w apps/web && npm run e2e -w apps/web
cargo test -p doto-server
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

# HANDOFF — Claude Code

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent claude --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-27 · antigravity)
**Đã làm:**
- Hoàn tất **P7 → P13 (`v0.2.0`)** đồng bộ 100% giữa bản App và bản Web (`ADR-014`): duyệt thư mục máy chủ (`/api/fs/dirs`), tải media từ điện thoại / App Photos + lưu về App Photos, import skill `.zip`/`.tar.gz` & tải thư mục trình duyệt, **AI Studio** điều phối đơn/đa phiên song song (`Antigravity` `agy` / `Claude Code` `claude` / `OpenAI Codex` `codex`), **CRUD đa Cloudflare Tunnel** + kiểm tra JWT **Cloudflare Access** (`Cf-Access-Jwt-Assertion`), trung tâm **Shared Memory & Handoff 3 AI** trên UI, và Playwright E2E chạy trực tiếp trên binary `doto-server` thật trong CI.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`, `docs/SECURITY.md`.

**Tests (máy dev Windows 11, 2026-11-27):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel/Access trên domain thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho Claude Code
1. Nếu chủ sở hữu cung cấp OAuth App `client_id`: cấu hình `DOTO_GITHUB_CLIENT_ID` trong CI/Release workflow và kiểm chứng GitHub Device Flow với repo private.
2. Nếu cần xác minh chữ ký số RS256/ES256 trực tuyến từ JWKS `/cdn-cgi/access/certs` bên cạnh kiểm tra loopback `cloudflared` + claims `iss`/`aud`/`exp`, có thể mở rộng `verify_cf_access_jwt` trong `crates/server/src/public.rs`.
3. Khi có chứng chỉ ký số Apple Developer / Windows Authenticode, cấu hình GitHub Secrets theo `docs/RELEASE.md` và phát hành bản signed/notarized.

Cách chạy tiếp:
```
npm ci
npm run release-check
npm run dev:server   # mở URL desktop-token được in ra
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** 2026-11-27T01:41:50.399Z bởi `antigravity`
- Branch: `main` · HEAD: `24ff531b67c503a6154d9d836d229def5dfcde91`
- Working tree: sạch
- Tóm tắt phiên: Hoàn tất refactor module hóa & tái sử dụng code (ADR-015): tách routes/*, v02/*, studio/*, importer/*, components/*, gom doto_core::{paths,process} & useDebouncedValue, quy định bắt buộc cho mọi tính năng mới
- NEXT_ACTION: Tuân thủ chuẩn module hóa ADR-015 khi phát triển tính năng mới; kiểm thử thêm trên thiết bị di động vật lý và cấu hình B1/B2/B3 khi sẵn sàng

**Commit gần nhất:**
- 24ff531 refactor(arch): module hóa toàn diện Rust/React và chuẩn hóa tái sử dụng code (ADR-015) (2026-11-27)
- a80ca3a docs(handoff): đồng bộ trạng thái hoàn tất phát hành v0.2.0 cho 3 AI (2026-11-27)
- 8c2ecc6 docs(release): ghi nhận kết quả CI/Release v0.2.0 và checksum installers (2026-11-27)
- cef4894 Merge pull request #2 from trancaodai1651/feat/v0.2-studio-p7-p13 (2026-11-27)
- 871813d fix(system): khai báo kiểu tường minh cho Vec<GpuInfo> khi biên dịch doto-server trên Linux CI (2026-11-27)

**Lần chạy test gần nhất:**
Máy: Windows 11 Pro (x86_64, i7-6800K, 32 GB, RTX 2070 SUPER) · Rust 1.99.1 · Node 26.13 · FFmpeg 9.0.2 (gyan full) · 2026-11-27 · agent: antigravity

| Lệnh / Workflow | Kết quả |
| --- | --- |
| `cargo test --workspace` (DOTO_SECRETS=memory, DOTO_REQUIRE_FFMPEG=1) | ✅ 115/115 test xanh: core 13 · system 8 · media 13 + render_e2e 3 (FFmpeg thật) · skills 25 + service 2 · integrations 17 (`StudioRunner` song song + CRUD đa tunnel) · server 11 + api_security 3 (kiểm chứng đồng bộ Owner App↔Web, duyệt thư mục host, mobile upload, `.zip` Zip-Slip, AI Studio đa phiên, CRUD đa tunnel + Cloudflare Access JWT, Memory lọc bí mật, chặn Viewer 403/404) |
| `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check` | ✅ sạch (0 cảnh báo) |
| `npm run typecheck/lint/test -w apps/web` | ✅ 33/33 unit test (Vitest, kiểm tra đồng bộ từ điển VI/EN) |
| `npm run e2e -w apps/web` (Playwright Chromium, mock API) | ✅ 5/5 (`onboarding.spec.ts`, `skills.spec.ts`, `gallery.spec.ts`, `web-parity.spec.ts`, `studio-v02.spec.ts`) |
| `npm run e2e:real` (Playwright chạy trực tiếp trên binary `doto-server` thật) | ✅ 1/1 (`real-server.spec.ts`: desktop one-time token → setup → onboarding 8 bước → bật Public server → import `.zip` skill → ghi bàn giao Memory → tải ảnh mobile → ghép nối Viewer RBAC) |
| `node scripts/secrets-scan.mjs` + `node scripts/license-check.mjs` | ✅ 242 file sạch bí mật · 639 phụ thuộc không copyleft bắt buộc |
| `npm run desktop:build -- --bundles nsis,msi` (Windows x64, local `0.2.0`) | ✅ `Doto Skill_0.2.0_x64-setup.exe` 6.27 MiB (`ffda71bd…96b1f`) · `Doto Skill_0.2.0_x64_en-US.msi` 9.10 MiB (`e49b052c…715b5`) |
| GitHub Actions CI PR #2 (`45919102697`) & `main` (`cef4894`) | ✅ 4/4 job xanh: `Web` + E2E `doto-server` thật (Ubuntu) · `Rust (windows-latest)` · `Rust (macos-latest)` · `Secrets & licenses` |
| GitHub Actions Release `v0.2.0` (`45919537257`) | ✅ https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.2.0 — `Doto.Skill_0.2.0_x64-setup.exe` (`c06d34a3…01c4f`) · `Doto.Skill_0.2.0_x64_en-US.msi` (`c3398d64…e8d95`) · `Doto.Skill_0.2.0_universal.dmg` (`26346cae…38921`, `lipo` `x86_64`+`arm64`) · `Doto-Skill_universal.app.zip` (`6b9adbad…7ebdc`) · `SHA256SUMS.txt` (`c6d68e4f…7956b`) · `THIRD_PARTY_LICENSES.tsv` (`995e5927…2e82b`) |
<!-- AUTO:END -->

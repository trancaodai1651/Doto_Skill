# HANDOFF — Google Antigravity

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent antigravity --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-27 · antigravity)
**Đã làm:**
- Hoàn tất triển khai trọn bộ **P7 → P13 lên bản `v0.2.0`** trên nền Rust/Tauri/Axum + React 19, giữ vững đồng nhất 100% tính năng giữa bản Desktop App và bản Web (`ADR-013`, `ADR-014`).
- **P7:** Duyệt thư mục máy chủ an toàn (`GET /api/fs/dirs`, `POST /api/fs/mkdir`, `<FolderPickerModal />`), tải ảnh/video trực tiếp từ điện thoại / App Photos (`POST /api/videos/upload` stream tới 1 GB, tự động lập chỉ mục video nguồn) và nút **Lưu về App Photos / Chia sẻ** (`navigator.share({ files })`).
- **P8:** Giải nén `.zip` thuần Rust (kiểm CRC-32, chặn Zip-Slip & zip bomb), `.tar.gz`/`.tar`, fallback `tar` hệ thống (`.7z`/`.rar` kèm dọn symlink), tải thư mục trình duyệt (`webkitdirectory` → `POST /api/skill-sources/upload-folder`) và tuỳ chọn tự tạo `SKILL.md` tối thiểu.
- **P9:** **AI Studio** (`crates/integrations/src/studio.rs` + trang `/studio`) điều phối chạy đơn phiên, đa phiên cùng 1 AI và đa AI song song (`Antigravity` `agy`, `Claude Code` `claude`, `OpenAI Codex` `codex`) không qua shell, truyền prompt nhiều dòng qua `stdin` (tránh lỗi `.cmd` CVE-2024-24576 trên Windows), tách thư mục xuất cô lập và tự lập chỉ mục MP4 mới tạo.
- **P10:** Quản lý **CRUD đa Cloudflare Tunnel** (`managed_tunnels` + OS Vault `cloudflare.tunnel.<id>` riêng từng tunnel, nút Quick Web Tunnel, dò/dừng tiến trình `cloudflared` ngoài) + xác thực JWT `Cf-Access-Jwt-Assertion` của **Cloudflare Access** trên listener Public.
- **P11 & P12:** Trung tâm **Shared Memory & Handoff 3 AI** trên UI (`/memory`, allowlist tệp `.md`, tự lọc `[REDACTED]`) + bộ test E2E chạy trực tiếp trên binary `doto-server` thật trong CI (`npm run e2e:real`).

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`, `docs/SECURITY.md`.

**Tests (máy dev Windows 11, 2026-11-27):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow (đã có `gh` CLI thay thế) · B2 tài khoản Cloudflare để kiểm chứng tunnel/Access trên domain sản xuất · B3 chứng chỉ ký số Authenticode / Apple notarization.

## Việc tiếp theo gợi ý cho Google Antigravity
1. Kiểm thử thực tế trên thiết bị di động vật lý cùng Wi-Fi (upload App Photos & lưu MP4 qua bảng Chia sẻ gốc của iOS/Android) và cập nhật `docs/shared/LAST_TEST_RUN.md`.
2. Khi chủ sở hữu cung cấp tài khoản Cloudflare + chứng chỉ ký mã, kiểm chứng Named Tunnel + Cloudflare Access trên domain thật và bật ký số/notarize trong `.github/workflows/release.yml`.

Rules workspace chính thức: `.agents/rules/doto-skill.md`.

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

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
| GitHub Actions CI PR #2 (`45919102697`) & `main` (`cef4894`) | ✅ 4/4 job xanh: `Web` + E2E `doto-server` thật (Ubuntu) · `Rust (windows-latest)` · `Rust (macos-latest)` · `Secrets & licenses` |
| GitHub Actions Release `v0.2.0` (`45919537257`) | ✅ https://github.com/trancaodai1651/Doto_Skill/releases/tag/v0.2.0 — `Doto.Skill_0.2.0_x64-setup.exe` (`c06d34a3…01c4f`) · `Doto.Skill_0.2.0_x64_en-US.msi` (`c3398d64…e8d95`) · `Doto.Skill_0.2.0_universal.dmg` (`26346cae…38921`, `lipo` `x86_64`+`arm64`) · `Doto-Skill_universal.app.zip` (`6b9adbad…7ebdc`) · `SHA256SUMS.txt` (`c6d68e4f…7956b`) · `THIRD_PARTY_LICENSES.tsv` (`995e5927…2e82b`) |

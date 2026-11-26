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

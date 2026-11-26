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

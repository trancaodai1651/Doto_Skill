# ROADMAP — Doto Skill

Trạng thái: ✅ đạt (có bằng chứng lệnh/test) · 🟡 một phần · ⏳ chưa làm · ⛔ bị chặn (ghi blocker)

| Phase | Nội dung | Phụ thuộc | Tiêu chí hoàn thành | Trạng thái |
| --- | --- | --- | --- | --- |
| P0 | Repo & foundation: monorepo, toolchain, code style, CI, SQLite migrations, design system, test skeleton, README | — | `cargo test` + `npm test` xanh; CI workflow xanh Win/macOS/Ubuntu | ✅ |
| P1 | Desktop/Server/Web: Tauri + Axum thật, UI responsive, đồng bộ 100% tính năng App ↔ Web cho Owner, kết nối URL server bản App, auth/session, LAN, firewall guide, tray/background | P0 | API security + E2E parity xanh; demo LAN thật | ✅ |
| P2 | Hardware & Resource: probe thật, 3 profile ảnh hưởng scheduler, metrics realtime, probe + benchmark 720p encoder | P0 | test resources + probe/benchmark NVENC thật | ✅ |
| P3 | CLI & Skills: onboarding, detect/install 4 công cụ, nhiều thư mục, GitHub public/private, OS vault, validation, mapping provider | P1 | test skills/service + import GitHub thật | 🟡 Device Flow cần OAuth client_id (blocker B1); private repo qua `gh` CLI đã có |
| P4 | Video E2E: import, ffprobe, thumbnail, render, queue, H.264, library, playback range/download, danh sách/thu hồi share link, RBAC | P1, P2 | render_e2e + api_security + E2E trình duyệt | ✅ |
| P5 | Cloudflare & hardening: cloudflared manager, CRUD đa tunnel, Cloudflare Access JWT, pairing/revoke, backup/restore SQLite, audit, secrets scan, license check | P1 | test tunnel/cf-access/pairing/backup-restore; secrets-scan + license-check xanh | ✅ (code + kiểm thử tự động 100%; kiểm chứng domain thật chờ tài khoản Cloudflare B2) |
| P6 | Release & handoff v0.1.0: đóng gói Win/macOS, GitHub Actions, tài liệu, release notes, checksum, handoff, push & GitHub Release `v0.1.0` | P0–P5 | Release workflow xanh, asset + SHA256SUMS trên GitHub Release | ✅ (đã phát hành `v0.1.0` unsigned kèm SHA256SUMS) |
| P7 | Duyệt thư mục máy chủ an toàn (`/api/fs/dirs`, `/api/fs/mkdir`) + tải ảnh/video từ điện thoại / App Photos (`/api/videos/upload`) & lưu về App Photos (`navigator.share`) | P1, P4 | API security + E2E thật kiểm chứng duyệt thư mục & upload | ✅ |
| P8 | Import Skill từ tệp nén `.zip`/`.tar.gz`/`.tgz`/`.tar`/`.7z`/`.rar` (kiểm CRC-32, chặn Zip-Slip) & tải thư mục từ trình duyệt (`webkitdirectory`) | P3 | unit test `importer.rs` + E2E `.zip` thật | ✅ |
| P9 | AI Studio: chạy đơn phiên & đa phiên song song (`Antigravity` `agy` / `Claude Code` `claude` / `OpenAI Codex` `codex`), đối số cố định không qua shell, thư mục xuất cô lập, tự lập chỉ mục video MP4 | P2, P3, P4 | unit test `studio.rs` + integration test `api_security.rs` + E2E | ✅ |
| P10 | Trình quản lý CRUD đa Cloudflare Tunnel (token lưu OS Vault riêng từng tunnel, nút Quick Web Tunnel, dò/dừng tiến trình `cloudflared` ngoài) + bắt buộc Cloudflare Access (`Cf-Access-Jwt-Assertion`) | P5 | unit test `tunnel.rs` + integration test `Cf-Access-Jwt-Assertion` | ✅ |
| P11 | Trung tâm Shared Memory & Handoff 3 AI ngay trên giao diện App & Web (`/api/memory/*`, allowlist tệp `.md`, tự động lọc `[REDACTED]`) | P1 | unit test `memory.rs` + integration test + E2E thật | ✅ |
| P12 | Bộ kiểm thử toàn diện (Rust unit + HTTP security + FFmpeg thật) & Playwright E2E chạy trực tiếp trên binary `doto-server` thật trong CI | P7–P11 | 115 test Rust + 33 unit Web + 5 E2E mock + 1 E2E `doto-server` thật xanh | ✅ |
| P13 | Đóng gói `v0.2.0`, CI đa nền tảng (Ubuntu + Windows + macOS) & phát hành GitHub Release `v0.2.0` kèm installers + `SHA256SUMS.txt` + đồng bộ 3 file handoff | P7–P12 | Release workflow xanh, asset `v0.2.0` trên GitHub Release | ✅ |

## Sau v0.2.0 (đề xuất)
- Ký số Windows (Authenticode) + Apple notarization khi chủ sở hữu cung cấp chứng chỉ (Blocker B3).
- mDNS (`doto.local`) cho khám phá tự động trên LAN.

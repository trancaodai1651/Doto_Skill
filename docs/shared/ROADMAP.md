# ROADMAP — Doto Skill

Trạng thái: ✅ đạt (có bằng chứng lệnh/test) · 🟡 một phần · ⏳ chưa làm · ⛔ bị chặn (ghi blocker)

| Phase | Nội dung | Phụ thuộc | Tiêu chí hoàn thành | Trạng thái |
| --- | --- | --- | --- | --- |
| P0 | Repo & foundation: monorepo, toolchain, code style, CI, SQLite migrations, design system, test skeleton, README | — | `cargo test` + `npm test` xanh; CI workflow có | ✅ (CI cục bộ; CI GitHub xem TASK_BOARD) |
| P1 | Desktop/Server/Web: Tauri + Axum thật, UI responsive, auth/session, start/stop, LAN, healthcheck, firewall guide, tray/background | P0 | API security test xanh; demo LAN thật | ✅ |
| P2 | Hardware & Resource: probe thật, 3 profile ảnh hưởng scheduler, metrics realtime, probe encoder | P0 | test resources + probe NVENC thật | ✅ |
| P3 | CLI & Skills: onboarding, detect/install 4 công cụ, nhiều thư mục, GitHub public/private, OS vault, validation, mapping provider | P1 | test skills/service + import GitHub thật | 🟡 Device Flow cần OAuth client_id (blocker B1); private repo qua `gh` CLI đã có |
| P4 | Video E2E: import, ffprobe, thumbnail, render, queue, H.264, library, playback range/download, RBAC | P1, P2 | render_e2e + api_security + demo trình duyệt | ✅ |
| P5 | Cloudflare & hardening: cloudflared manager, named tunnel, Access guidance, pairing/revoke, audit, secrets scan, license check | P1 | test tunnel/pairing; secrets-scan + license-check xanh | 🟡 chạy tunnel thật với Cloudflare account chưa kiểm chứng (blocker B2) |
| P6 | Release & handoff: đóng gói Win/macOS, GitHub Actions, tài liệu, release notes, checksum, handoff, push | P0–P5 | Release workflow xanh, asset + SHA256SUMS trên GitHub Release | xem TASK_BOARD |

## Sau v0.1.0 (đề xuất)
- Ký số Windows (Authenticode) + Apple notarization khi có chứng chỉ.
- Cloudflare Access tích hợp kiểm tra JWT `Cf-Access-Jwt-Assertion` (tuỳ chọn) cho listener Public.
- Giao diện quản lý share link; benchmark encoder tuỳ chọn; mDNS (`doto.local`).
- AI editing qua skill/agent (job kind mới) — chưa triển khai, không quảng cáo.
- Test Playwright E2E chạy trên server Rust thật trong CI (hiện E2E UI dùng mock + test HTTP Rust riêng).

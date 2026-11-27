# CHANGELOG_DEV — thay đổi đã kiểm thử

Mỗi mục: ngày · agent · commit · tóm tắt (lệnh/kết quả chi tiết nằm trong `LAST_TEST_RUN.md` tại thời điểm đó).

## 2026-11-26 · claude · v0.1.0 nền tảng
- Khởi tạo monorepo Rust/Tauri/React; chuyển bản Node v0.2 vào `legacy/studio-v0.2/`.
- Toàn bộ P0–P4 + phần lớn P5/P6 (xem `TASK_BOARD.md`).
- Lệnh tái lập: `npm ci && npm run release-check` (cần ffmpeg/ffprobe), `npm run e2e`.
- Sửa trong phiên sau khi chạy thật: Codex in `login status` ra stderr; lọc interface LAN quá chặt (bỏ sót CGNAT/public) → thêm `scope` + cảnh báo; đổi cổng mặc định 4320→4380; cửa sổ desktop bắt buộc tạo Owner khi chưa có mật khẩu; tiêu đề "0 skills" ẩn khi đã có nguồn.

- 2026-11-26 · claude · `347ef959a1` · Nền tảng v0.1.0: 6 crate Rust + Tauri + web; 101 test Rust + 32 unit + 3 e2e web xanh cục bộ; installer Windows build cục bộ; PR #1 đang chạy CI

- 2026-11-26 · antigravity · `0d13454be2` · Đồng bộ 100% tính năng bản App & bản Web (Owner dùng phần cứng server bản App), thêm kết nối URL server, CORS Bearer + media ticket, sao lưu/khôi phục SQLite, benchmark encoder 720p, danh sách share link; 101 test Rust + 33 unit + 4 E2E xanh, đóng gói installer Windows

- 2026-11-26 · antigravity · `0592add8c0` · Hoàn tất P0–P6 & đồng bộ 100% tính năng bản App ↔ bản Web (Owner kết nối phần cứng server bản App); merge PR #1 vào main và phát hành thành công GitHub Release v0.1.0 (Win NSIS/MSI + macOS universal DMG/app.zip + SHA256SUMS)

- 2026-11-27 · antigravity · `7e05d5e395` · Triển khai trọn bộ P7–P13 lên v0.2.0 (AI Studio đa phiên Antigravity/Claude/Codex, Mobile Upload & App Photos, Import Skill .zip/folder, CRUD đa Tunnel + Cloudflare Access JWT, Trung tâm Memory/Handoff 3 AI, E2E doto-server thật)

- 2026-11-27 · antigravity · `8c2ecc6fe1` · Hoàn tất P0–P13 (v0.2.0): merge PR #2 vào main (cef4894) và phát hành thành công GitHub Release v0.2.0 (45919537257 — Win NSIS/MSI + macOS Universal DMG/app.zip + SHA256SUMS)

# CHANGELOG_DEV — thay đổi đã kiểm thử

Mỗi mục: ngày · agent · commit · tóm tắt (lệnh/kết quả chi tiết nằm trong `LAST_TEST_RUN.md` tại thời điểm đó).

## 2026-11-26 · claude · v0.1.0 nền tảng
- Khởi tạo monorepo Rust/Tauri/React; chuyển bản Node v0.2 vào `legacy/studio-v0.2/`.
- Toàn bộ P0–P4 + phần lớn P5/P6 (xem `TASK_BOARD.md`).
- Lệnh tái lập: `npm ci && npm run release-check` (cần ffmpeg/ffprobe), `npm run e2e`.
- Sửa trong phiên sau khi chạy thật: Codex in `login status` ra stderr; lọc interface LAN quá chặt (bỏ sót CGNAT/public) → thêm `scope` + cảnh báo; đổi cổng mặc định 4320→4380; cửa sổ desktop bắt buộc tạo Owner khi chưa có mật khẩu; tiêu đề "0 skills" ẩn khi đã có nguồn.

- 2026-11-26 · claude · `347ef959a1` · Nền tảng v0.1.0: 6 crate Rust + Tauri + web; 101 test Rust + 32 unit + 3 e2e web xanh cục bộ; installer Windows build cục bộ; PR #1 đang chạy CI

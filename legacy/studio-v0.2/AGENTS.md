# Doto Skill Studio — Quy chuẩn Điều phối Đa AI (Antigravity · Claude Code · Codex)

> **Đọc bắt buộc trước khi sửa mã hoặc chạy lệnh:**
> 1. [MEMORY.md](./MEMORY.md) — Bộ nhớ dài hạn dùng chung (kiến trúc, quy ước không phụ thuộc npm ở `server/`, giải nén `.zip`/`.rar`, Cloudflare Tunnel CRUD, bẫy kỹ thuật Windows/iOS).
> 2. [HANDOFF.md](./HANDOFF.md) — Trạng thái bàn giao phiên hiện tại, danh sách tính năng vừa cập nhật và lệnh kiểm thử/đóng gói.
> 3. Nếu tác vụ liên quan tới nội dung hoặc engine của 6 Skill dựng video gốc, đọc thêm `E:/_TRAN CAO DAI/SKILL/AGENTS.md`, `E:/_TRAN CAO DAI/SKILL/MEMORY.md` và `E:/_TRAN CAO DAI/SKILL/HANDOFF.md`.
> 4. **Sau khi hoàn thành công việc:** Luôn chạy đủ `npm.cmd run typecheck` + `node --test test/*.test.mjs`, cập nhật `.desktop/` nếu sửa backend/frontend và ghi tóm tắt bàn giao vào `HANDOFF.md` (cùng `MEMORY.md` nếu có quy ước mới).

## Cấu trúc thư mục chính
| Đường dẫn | Vai trò |
| --- | --- |
| `server/index.mjs` | Máy chủ HTTP + SSE + Byte-Range Media + API hợp nhất (port mặc định `4320`) |
| `server/skill-importer.mjs` | Giải nén & cài đặt Skill từ `.zip`, `.rar`, `.7z`, `.tar.gz` hoặc thư mục + gắn junction 3 AI |
| `server/runner.mjs` | Điều phối tiến trình đơn phiên & đa phiên song song trên cùng 1 AI hoặc nhiều AI |
| `server/tunnels.mjs` | Quản lý CRUD Cloudflare Tunnel + quét tiến trình `cloudflared` trên thiết bị |
| `server/media-upload.mjs` | Nhận luồng ảnh/video từ App Photos trên iPad, iPhone, Android |
| `server/memory-handoff.mjs` | Đồng bộ & biên tập `MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` |
| `src/App.tsx` & `src/components/*` | Giao diện React 19 hỗ trợ Sáng/Tối, VI/EN, tối ưu PC · iPad · iPhone |
| `src-tauri/` & `scripts/build-desktop.mjs` | Đóng gói ứng dụng Desktop Tauri v2 cho Windows NSIS `.exe` & macOS Universal |

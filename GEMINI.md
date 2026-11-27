# GEMINI.md — Google Antigravity

Đọc `AGENTS.md`, rồi `docs/shared/PROJECT_MEMORY.md`, `docs/shared/ROADMAP.md`, `docs/shared/TASK_BOARD.md` và `docs/agents/HANDOFF_ANTIGRAVITY.md` trước khi sửa code.
Quy tắc workspace chính thức nằm ở `.agents/rules/doto-skill.md` (có frontmatter `trigger`).
Khi thêm tính năng mới, bắt buộc tuân thủ chuẩn **Module hóa & Tái sử dụng code (`ADR-015`)** trong `AGENTS.md` (giữ file gọn < ~600 dòng, tách submodule chuyên trách, dùng lại `doto_core::{paths, process}` và `apps/web/src/{lib,components}`).
Sau mỗi mốc: `npm run handoff -- --agent antigravity --summary "..." --next "..."`.

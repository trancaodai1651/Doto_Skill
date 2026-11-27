# CLAUDE.md — Claude Code

@AGENTS.md
@docs/shared/PROJECT_MEMORY.md
@docs/shared/TASK_BOARD.md
@docs/agents/HANDOFF_CLAUDE.md

Ghi chú riêng cho Claude Code:
- Trên Windows, công cụ Bash là Git Bash; ưu tiên đường dẫn tuyệt đối có dấu ngoặc kép (thư mục repo có dấu cách).
- Tuân thủ chuẩn Module hóa & Tái sử dụng code (`ADR-015` trong `AGENTS.md`): không tạo file nguyên khối, luôn dùng lại tiện ích trong `doto_core::{paths, process}` và `apps/web/src/{lib,components}`.
- Sau mỗi mốc: `npm run handoff -- --agent claude --summary "..." --next "..."`.

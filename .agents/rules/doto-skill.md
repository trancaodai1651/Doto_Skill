---
trigger: always_on
description: Quy tắc chung của dự án Doto Skill — đọc nguồn sự thật chung trước khi sửa code và cập nhật bàn giao sau khi làm.
---

# Doto Skill — quy tắc workspace (Antigravity)

1. Trước khi sửa code: đọc `AGENTS.md`, `docs/shared/PROJECT_MEMORY.md`, `docs/shared/ROADMAP.md`, `docs/shared/TASK_BOARD.md`, `docs/agents/HANDOFF_ANTIGRAVITY.md`.
2. Kiến trúc: Rust (crates/*) + Tauri v2 (apps/desktop) + React (apps/web). Browser client không bao giờ được chạy lệnh shell/CLI hay đọc filesystem host — route điều khiển máy chỉ có trên listener Control của desktop.
3. Không cài sẵn/không tự tải skill; skill import là input không tin cậy, không tự chạy script.
4. Không lưu token/mật khẩu vào SQLite, log hay Git — dùng OS vault (`crates/core/src/secrets.rs`).
5. Sau khi làm: chạy test phù hợp, cập nhật `docs/shared/*` và chạy `npm run handoff -- --agent antigravity --summary "..." --next "..."` để đồng bộ cả 3 file handoff.

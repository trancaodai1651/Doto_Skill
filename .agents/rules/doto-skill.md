---
trigger: always_on
description: Quy tắc chung của dự án Doto Skill — đọc nguồn sự thật chung trước khi sửa code và cập nhật bàn giao sau khi làm.
---

# Doto Skill — quy tắc workspace (Antigravity)

1. Trước khi sửa code: đọc `AGENTS.md`, `docs/shared/PROJECT_MEMORY.md`, `docs/shared/ROADMAP.md`, `docs/shared/TASK_BOARD.md`, `docs/agents/HANDOFF_ANTIGRAVITY.md`.
2. Kiến trúc: Rust (crates/*) + Tauri v2 (apps/desktop) + React (apps/web). Bản App và bản Web có đầy đủ chức năng như nhau cho Owner (kết nối và dùng phần cứng máy chủ của bản App qua API allowlist an toàn). Browser client không bao giờ chạy lệnh shell/CLI tuỳ ý hay đọc trực tiếp filesystem ngoài phạm vi cấp quyền; Viewer chỉ xem/tải video được chia sẻ.
3. Không cài sẵn/không tự tải skill; skill import là input không tin cậy, không tự chạy script.
4. Không lưu token/mật khẩu vào SQLite, log hay Git — dùng OS vault (`crates/core/src/secrets.rs`).
5. Module hóa & tái sử dụng code (`ADR-015` — bắt buộc cho mọi tính năng mới): giữ file nguồn gọn (< ~600 dòng), tách theo thư mục submodule chuyên trách; dùng lại tiện ích nền tảng trong `doto_core::{paths, process}` (không lặp lại `home_dir`, `exe_name`, `creation_flags`), hook trong `apps/web/src/lib/useApi.ts` (`useApi`, `useAction`, `useDebouncedValue`) và component trong `packages/ui` + `apps/web/src/components/*`.
6. Sau khi làm: chạy test phù hợp, cập nhật `docs/shared/*` và chạy `npm run handoff -- --agent antigravity --summary "..." --next "..."` để đồng bộ cả 3 file handoff.

# AGENTS.md — Doto Skill (Codex · Claude Code · Antigravity)

> Nguồn sự thật chung nằm trong `docs/shared/` và `docs/agents/` — **không** phụ thuộc context tạm của một CLI.

## Bắt buộc trước khi sửa code
1. Đọc `docs/shared/PROJECT_MEMORY.md` (kiến trúc, quyết định, quy tắc).
2. Đọc `docs/shared/ROADMAP.md` và `docs/shared/TASK_BOARD.md`.
3. Đọc file bàn giao của bạn: `docs/agents/HANDOFF_CODEX.md` (Codex) · `HANDOFF_CLAUDE.md` (Claude Code) · `HANDOFF_ANTIGRAVITY.md` (Antigravity).

## Bắt buộc sau khi làm
1. Chạy kiểm thử phù hợp (tối thiểu `npm run unit-test`; trước khi phát hành: `npm run release-check`) và ghi kết quả vào `docs/shared/LAST_TEST_RUN.md`.
2. Cập nhật `PROJECT_MEMORY.md` (nếu có quyết định/quy ước mới), `DECISIONS.md` (ADR), `TASK_BOARD.md`.
3. Chạy `npm run handoff -- --agent <codex|claude|antigravity> --summary "..." --next "..."` để đồng bộ **cả 3** file handoff.
4. Không đánh dấu xong khi chưa đạt tiêu chí nghiệm thu. Không ghi token, mật khẩu, prompt riêng tư, nội dung repo private vào bất kỳ file nào.

## Cấu trúc & Chuẩn Module Hóa / Tái Sử Dụng Code (BẮT BUỘC khi thêm tính năng mới)
- **Nguyên tắc Module hóa (`ADR-015`):** Không viết file đơn nguyên khối (monolithic). Mỗi file nguồn nên giữ gọn trong khoảng **200–500 dòng** (tối đa không quá ~650 dòng); khi một module lớn hơn, bắt buộc tách thành thư mục submodule chuyên trách (ví dụ `crates/server/src/routes/{mod,auth_system,skills_tools,media_jobs}.rs`, `crates/server/src/v02/{mod,fs_uploads,skills_studio}.rs`, `crates/integrations/src/studio/{mod,cli}.rs`, `crates/skills/src/importer/{mod,archive}.rs`).
- **Tăng cường tái sử dụng (Reuse-first):**
  - **Rust:** Mọi tiện ích hệ điều hành, đường dẫn, kiểm tra tên tệp đơn, tạo tiến trình con ẩn console (`CREATE_NO_WINDOW`), lọc bí mật, băm SHA-256, sinh ID/token phải đặt tại `crates/core/src/{paths,process,lib}.rs` và tái sử dụng xuyên suốt các crate — tuyệt đối không copy-paste hàm `home_dir`, `exe_name`, `creation_flags(0x0800_0000)` hay `audit_ev` ở nhiều nơi.
  - **React / TypeScript:** Mọi UI primitive đặt ở `packages/ui/src/*`; hook dùng chung (`useApi`, `useAction`, `useDebouncedValue`) đặt ở `apps/web/src/lib/*`; các khối giao diện nghiệp vụ có thể tái sử dụng hoặc tách từ trang lớn đặt ở `apps/web/src/components/{common,features,video,tunnels,skill-sources,FolderPicker}.tsx`. Khi thêm tính năng mới, phải kiểm tra và dùng lại hook/component sẵn có trước khi tạo mới.

| Đường dẫn | Vai trò |
| --- | --- |
| `crates/core` | SQLite + migrations, settings, audit, jobs, event bus, OS secret vault, tiện ích `paths` & `process` dùng chung |
| `crates/system` | HardwareProbe, ResourceManager (profile Nhẹ/Cân bằng/Tối đa), admission control |
| `crates/media` | FFmpeg/ffprobe, probe & benchmark encoder thật, render queue (trim/ghép → MP4 H.264/AAC) |
| `crates/skills` | Nguồn skill thư mục + GitHub + `importer/{mod,archive}` (`.zip`/`.tar.gz`/folder upload), scanner an toàn, mapping sang provider |
| `crates/integrations` | `CliProvider` + adapter Antigravity/Codex/Claude/cloudflared, installer, `studio/{mod,cli}`, Multi-Tunnel `TunnelManager` |
| `crates/server` | Axum: auth/RBAC, listener Control + Public, Range streaming, SSE, `routes/{auth_system,skills_tools,media_jobs}`, `v02/{fs_uploads,skills_studio}`, `memory` (`docs/API.md`) |
| `apps/desktop/src-tauri` | Tauri v2: chạy server in-process, tray, chạy nền, autostart |
| `apps/web` · `packages/ui` | React 19 + TS + Vite + Tailwind 4, design system & các module component chuyên biệt dùng chung |
| `scripts/` | `task.mjs` (format/lint/typecheck/test/build/release-check), `update-handoff.mjs`, quét bí mật, kiểm tra license |
| `legacy/studio-v0.2/` | Mã nguồn bản Node v0.2 trước đây — chỉ tham khảo, không phát triển tiếp |

## Lệnh nhanh (Windows: dùng `npm.cmd` nếu PowerShell chặn `npm.ps1`)
```
npm ci
npm run unit-test
npm run integration-test   # cần ffmpeg + ffprobe trên PATH
npm run e2e
npm run release-check
npm run dev:server          # API + UI tại http://127.0.0.1:4319 (in URL đăng nhập desktop)
npm run desktop:build
```

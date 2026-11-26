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

## Cấu trúc
| Đường dẫn | Vai trò |
| --- | --- |
| `crates/core` | SQLite + migrations, settings, audit, jobs, event bus, OS secret vault |
| `crates/system` | HardwareProbe, ResourceManager (profile Nhẹ/Cân bằng/Tối đa), admission control |
| `crates/media` | FFmpeg/ffprobe, probe encoder thật, render queue (trim/ghép → MP4 H.264/AAC) |
| `crates/skills` | Nguồn skill thư mục + GitHub, scanner an toàn, mapping sang provider |
| `crates/integrations` | `CliProvider` + adapter Antigravity/Codex/Claude/cloudflared, installer, TunnelManager |
| `crates/server` | Axum: auth/RBAC, listener Control + Public, Range streaming, SSE, API (`docs/API.md`) |
| `apps/desktop/src-tauri` | Tauri v2: chạy server in-process, tray, chạy nền, autostart |
| `apps/web` · `packages/ui` | React 19 + TS + Vite + Tailwind 4, design system dùng chung |
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

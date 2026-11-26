# HANDOFF — OpenAI Codex

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent codex --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · claude)
**Đã làm:** Khởi tạo Doto Skill v0.1.0 trên nền Rust/Tauri/React (P0–P4 đạt; P5/P6 phần lớn). Bản Node v0.2 cũ được giữ tại `legacy/studio-v0.2/`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho OpenAI Codex
1. UI quản lý share link trong `apps/web/src/pages/Videos.tsx` (API: `POST /api/videos/:id/share {ttlMinutes}` → `{id,url,expiresAt}`, `DELETE /api/shares/:id`). Thêm type `ShareLink` đã có trong `apps/web/src/api/types.ts`.
2. Playwright E2E chạy với server Rust thật: khởi động `target/debug/doto-server --data-dir .e2e-data --control-port 0 --token-file .e2e-data/url.txt --web-root apps/web/dist` trong `globalSetup`, dùng URL token để đăng nhập desktop.
3. Giữ hợp đồng: mọi thay đổi field API phải sửa đồng thời `apps/web/src/api/types.ts`, handler trong `crates/server/src/routes.rs` và `docs/API.md`.

Cách chạy tiếp:
```
npm ci
npm run test -w apps/web && npm run e2e -w apps/web
cargo test -p doto-server
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
<!-- AUTO:END -->

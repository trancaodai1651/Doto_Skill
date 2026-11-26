# HANDOFF — Claude Code

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent claude --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · claude)
**Đã làm:** Khởi tạo Doto Skill v0.1.0 trên nền Rust/Tauri/React (P0–P4 đạt; P5/P6 phần lớn). Bản Node v0.2 cũ được giữ tại `legacy/studio-v0.2/`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho Claude Code
1. Theo dõi CI của PR `feat/initial-platform` trên GitHub (Windows + macOS); sửa lỗi riêng macOS nếu có (keyring Keychain trong test dùng `DOTO_SECRETS=memory`).
2. Khi CI xanh: merge → tag `v0.1.0` → kiểm tra Release workflow, tải asset và đối chiếu `SHA256SUMS.txt`.
3. Nếu người dùng cung cấp OAuth App client_id: thêm biến `DOTO_GITHUB_CLIENT_ID` (repo variable) vào bước build release và kiểm chứng Device Flow với repo private.

Cách chạy tiếp:
```
npm ci
npm run release-check
npm run dev:server   # mở URL desktop-token được in ra
```

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
<!-- AUTO:END -->

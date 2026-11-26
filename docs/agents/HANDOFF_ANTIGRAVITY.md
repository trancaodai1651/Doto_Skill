# HANDOFF — Google Antigravity

> Đọc trước: `AGENTS.md` → `docs/shared/PROJECT_MEMORY.md` → `docs/shared/ROADMAP.md` → `docs/shared/TASK_BOARD.md` → file này.
> Sau khi làm: cập nhật `docs/shared/*` rồi `npm run handoff -- --agent antigravity --summary "..." --next "..."` (đồng bộ cả 3 file handoff).

## Phiên gần nhất (2026-11-26 · claude)
**Đã làm:** Khởi tạo Doto Skill v0.1.0 trên nền Rust/Tauri/React (P0–P4 đạt; P5/P6 phần lớn). Bản Node v0.2 cũ được giữ tại `legacy/studio-v0.2/`.

**File/module cốt lõi:** `crates/{core,system,media,skills,integrations,server}`, `apps/desktop/src-tauri`, `apps/web`, `packages/ui`, `scripts/*.mjs`, `.github/workflows/{ci,release}.yml`, `docs/API.md`.

**Tests (máy dev Windows 11, 2026-11-26):** xem khối "Lần chạy test gần nhất" bên dưới và `docs/shared/LAST_TEST_RUN.md`.

**Blocker:** B1 OAuth client_id cho GitHub Device Flow · B2 tài khoản Cloudflare để kiểm chứng tunnel thật · B3 chứng chỉ ký mã (chi tiết: `TASK_BOARD.md`).

## Việc tiếp theo gợi ý cho Google Antigravity
1. Kiểm thử thủ công trên máy người dùng: cài installer Windows từ GitHub Release, chạy onboarding, bật LAN trên interface mạng riêng, mở trên điện thoại cùng Wi-Fi bằng QR/pairing, phát + tua + tải video. Ghi kết quả vào `docs/shared/LAST_TEST_RUN.md`.
2. Kiểm chứng Cloudflare: Quick Tunnel (thử) và Named Tunnel với token + hostname thật; xác nhận SSE fallback polling trên quick tunnel; khuyến nghị bật Cloudflare Access.
3. Nếu chỉnh skill dựng video gốc tại `E:/_TRAN CAO DAI/SKILL`, thêm thư mục đó làm nguồn Skills trong app (không commit nội dung skill riêng tư vào repo public).

Rules workspace chính thức: `.agents/rules/doto-skill.md`.

<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
<!-- AUTO:END -->

# HANDOFF.md — Bàn giao liên phiên (Google Antigravity · Claude Code · OpenAI Codex)

> Cập nhật tệp này cuối mỗi phiên làm việc để bất kỳ Agent nào (**Antigravity**, **Claude Code**, **Codex**) vào sau đều nắm bắt tức thì trạng thái dự án **Doto Skill Studio**.
> Quy ước kiến trúc lâu dài & bẫy kỹ thuật xem tại [MEMORY.md](./MEMORY.md). Bộ Skill gốc nằm tại `E:\_TRAN CAO DAI\SKILL` (xem `E:\_TRAN CAO DAI\SKILL\HANDOFF.md`).

## Trạng thái hiện tại (2026-11-26 · Phiên bản v0.2.0 · Antigravity)

### Đã hoàn thành trọn vẹn trong phiên v0.2.0
1. **Import Skill vào App (`server/skill-importer.mjs` + `src/components/SkillImportModal.tsx`)**:
   - Hỗ trợ nhập từ **file nén `.zip`, `.rar`, `.7z`, `.tar.gz`**, tải lên **cả thư mục** từ trình duyệt (`webkitdirectory`) hoặc chọn đường dẫn cục bộ trên PC.
   - Tùy chỉnh nơi lưu (`customSkillSaveDir`), tự tạo `SKILL.md` nếu gói thiếu frontmatter và tự tạo directory junction sang `~/.claude/skills`, `~/.codex/skills`, `~/.gemini/skills`, `~/.gemini/antigravity-cli/skills`.
2. **Tùy chỉnh nơi lưu (`server/data.mjs`, `server/videos.mjs`, `src/App.tsx`)**:
   - Cho phép chọn/đổi nơi lưu Skill mới, nơi lưu dự án/video xuất ra (`videoOutputDir`) và nơi lưu ảnh/video tải từ điện thoại (`mediaUploadDir`) kèm nút duyệt thư mục hệ điều hành (`/api/pick-path`).
3. **Chạy đa phiên song song trên cùng 1 AI hoặc đồng thời nhiều AI (`POST /api/jobs/batch` + màn hình chia lưới)**:
   - Hỗ trợ 4 chế độ trong Studio: `1 Phiên Đơn`, `Nhiều Phiên · Cùng 1 AI` (Antigravity / Claude / Codex), `Nhiều Phiên · Nhiều AI` (chạy song song cả 3 AI, mỗi AI giữ Model + Effort riêng) và `Hàng Đợi Đa Phiên Tùy Chỉnh`.
   - Tự động tách hậu tố thư mục dự án chống xung đột file khi nhiều tiến trình render cùng lúc; bổ sung chế độ xem **Màn hình chia lưới Đa Phiên** thời gian thực.
4. **Giao diện Sáng / Tối & Song ngữ Tiếng Việt / Tiếng Anh (`src/styles.css`, `src/i18n.ts`)**:
   - Nút chuyển đổi nền **Sáng / Tối** và ngôn ngữ **VI / EN** ngay trên thanh công cụ, đồng bộ biến CSS toàn cục và lưu vào `settings.json` + `localStorage`.
5. **Tối ưu Web cho iPad, iPhone, PC & Tích hợp App Photos di động (`server/media-upload.mjs`, `src/App.tsx`)**:
   - Bố cục đáp ứng 3 ngưỡng (PC > 1180px, iPad 768–1180px, iPhone/Android <= 767px có thanh điều hướng đáy cố định + PWA `manifest.webmanifest`).
   - Mở trực tiếp Thư viện Ảnh (App Photos) trên iPhone/iPad/Android để tải ảnh/clip 4K đưa vào prompt AI, nút mở nhanh `photos-redirect://` và nút **Lưu Video MP4 về App Photos** (`navigator.share({ files })`).
6. **Quản lý CRUD Cloudflare Tunnel & Mở Tunnel cho Web (`server/tunnels.mjs`, `src/components/TunnelManagerTab.tsx`)**:
   - Đầy đủ Create / Read / Update / Delete + Start / Stop / Restart cho Quick Tunnel (`*.trycloudflare.com`), Zero Trust Token Tunnel (`--token`) và Named Tunnel.
   - Quét trực tiếp các tiến trình `cloudflared` đang chạy trên thiết bị để nhận nuôi (Adopt) hoặc dừng (Stop).
7. **Trung tâm Bộ nhớ chung & Bàn giao cho 3 AI (`server/memory-handoff.mjs`, `src/components/MemoryHandoffTab.tsx`)**:
   - Quản lý, đồng bộ và sửa trực tiếp 5 tệp (`MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`) ngay trên giao diện ứng dụng, tự động ghi nhật ký phiên hoàn tất vào `HANDOFF.md`.

## Lệnh kiểm tra & Đóng gói nhanh cho Agent tiếp theo
- **Bộ cài đặt Windows v0.2.0 đã xuất bản**: `src-tauri/target/release/bundle/nsis/Doto Skill_0.2.0_x64-setup.exe` (và file chạy độc lập `src-tauri/target/release/doto-skill.exe`).
```powershell
cd "E:\_TRAN CAO DAI\CODE\Doto_Skill"
npm.cmd run typecheck
node --test test/*.test.mjs
npm.cmd run build
npm.cmd run desktop:build
```

## Nhật ký
- 2026-11-26 · **Antigravity** · Nâng cấp lên **Doto Skill Studio v0.2.0**: Import Skill (`.zip`/`.rar`/folder) + tùy chỉnh nơi lưu, Đa phiên song song (cùng 1 AI & đa AI), nền Sáng/Tối, song ngữ VI/EN, tối ưu Web iPad/iPhone/PC + tích hợp App Photos, quản lý CRUD Cloudflare Tunnel, đồng bộ Bộ nhớ chung & Bàn giao cho Antigravity/Claude/Codex, xuất bản bộ cài `.exe` mới và mở Cloudflare Tunnel trực tiếp.
- 2026-11-26 · **Antigravity** · Khởi tạo phiên bản v0.1.0 ban đầu kết nối 6 Skill dựng video với Claude Code, Codex và Antigravity.

# Doto Skill Studio v0.2.0 (Windows · macOS Universal · Web cho PC / iPad / iPhone / Android)

Ứng dụng điều phối sản xuất video tự động đa phiên · đa AI (**Google Antigravity**, **Claude Code**, **OpenAI Codex**):

- **Import Skill đa năng**: Nhập từ **thư mục** hoặc **file nén (`.zip`, `.rar`, `.7z`, `.tar.gz`)**, tùy chỉnh nơi lưu & tự gắn liên kết (junction) cho cả 3 AI.
- **Tùy chỉnh nơi lưu**: Tùy chọn thư mục lưu Skill nhập vào, thư mục lưu dự án/video xuất ra và thư mục lưu ảnh/video tải từ điện thoại.
- **Đa Phiên · Đa AI Song Song**: Chạy cùng lúc nhiều phiên tạo video trên **cùng 1 AI** (Antigravity / Claude / Codex) hoặc **nhiều phiên trên nhiều AI đồng thời**, kèm màn hình giám sát chia lưới thời gian thực.
- **Nền Sáng / Tối & Song ngữ VI / EN**: Chuyển đổi tức thì giao diện Light / Dark và Tiếng Việt / English.
- **Tối ưu iPad · iPhone · Android · PC + App Photos**: Giao diện Web/PWA có thanh điều hướng đáy cho di động, chọn ảnh/video trực tiếp từ **App Photos** (iOS/iPadOS/Android) đưa vào video và lưu MP4 xuất ra về lại App Photos.
- **Quản lý CRUD Cloudflare Tunnel**: Mở nhanh Cloudflare Tunnel HTTPS công khai cho Web Studio và quản lý (Tạo / Xem / Sửa / Xoá / Bật / Tắt / Nhận nuôi) mọi tiến trình `cloudflared` trên thiết bị.
- **Bộ nhớ chung & Bàn giao 3 AI**: Quản lý trực tiếp `MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` để Antigravity, Claude Code và Codex phối hợp liền mạch.

---

## 1. Chạy phiên bản Web + Mở Cloudflare Tunnel

```powershell
cd "E:\_TRAN CAO DAI\CODE\Doto_Skill"
npm.cmd run build

# Chạy máy chủ mạng LAN + tự động mở Cloudflare Tunnel công khai:
node bin/cli.mjs --lan --tunnel
```

## 2. Xuất bản ứng dụng Windows Desktop (`.exe` NSIS Installer)

```powershell
npm.cmd run desktop:build
```
Bộ cài xuất tại: `src-tauri/target/release/bundle/nsis/Doto Skill_0.2.0_x64-setup.exe`.

## 3. Xuất bản ứng dụng macOS Universal (`arm64` + `x86_64`)

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run desktop:build:mac-universal
```

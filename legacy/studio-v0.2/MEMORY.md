# MEMORY.md — Bộ nhớ chung cho mọi Agent (Google Antigravity · Claude Code · OpenAI Codex)

> Trí nhớ dài hạn DÙNG CHUNG cho dự án **Doto Skill Studio** (`E:\_TRAN CAO DAI\CODE\Doto_Skill`), kết nối trực tiếp với kho Skill gốc tại `E:\_TRAN CAO DAI\SKILL`.
> Mọi Agent (**Antigravity**, **Claude Code**, **Codex**) đọc tệp này đầu phiên và ghi nối tiếp khi có quy ước, quyết định kiến trúc hoặc bẫy kỹ thuật mới.
> Trạng thái phiên làm việc hiện tại ghi tại `HANDOFF.md`.

## 1. Mục tiêu sản phẩm & Yêu cầu người dùng
- Ứng dụng điều phối sản xuất video tự động hợp nhất chạy trên **Windows Desktop (`.exe` Tauri v2 NSIS)**, **macOS Universal (`arm64` + `x86_64`)** và **Web / PWA** tối ưu cho **PC, iPad, iPhone và Android**.
- **Import Skill đa định dạng**: Nhập từ **thư mục** (local hoặc `webkitdirectory` từ trình duyệt) hoặc **file nén (`.zip`, `.rar`, `.7z`, `.tar.gz`)**, cho phép **tùy chỉnh nơi lưu** (`customSkillSaveDir`) và tự gắn junction vào `~/.claude/skills`, `~/.codex/skills`, `~/.gemini/skills`, `~/.gemini/antigravity-cli/skills`.
- **Tùy chỉnh nơi lưu dự án & video** (`videoOutputDir`) và thư mục lưu media di động (`mediaUploadDir`).
- **Đa phiên song song**: Chạy đồng thời **nhiều phiên tạo video trên cùng 1 AI** (`Antigravity` hoặc `Claude` hoặc `Codex`) HOẶC **nhiều phiên trên nhiều AI cùng lúc**, tự động tách hậu tố thư mục dự án (`-s1`, `-s2`...) chống đụng tệp khi render Remotion/HyperFrames song song.
- **Giao diện Sáng / Tối & Song ngữ**: Chuyển đổi tức thì nền **Sáng (`light`) / Tối (`dark`)** và ngôn ngữ **Tiếng Việt (`vi`) / Tiếng Anh (`en`)**.
- **Tích hợp App Photos di động (iPad · iPhone · Android)**: Mở trực tiếp Thư viện Ảnh (`accept="image/*,video/*" multiple`), chụp camera hoặc gọi `photos-redirect://` để lấy tư liệu thật đưa vào phiên AI; khi xuất video xong hỗ trợ **Lưu về App Photos** qua Web Share API Level 2 (`navigator.share({ files })`) hoặc tải luồng trực tiếp.
- **Quản lý CRUD Cloudflare Tunnel**: Tạo / Xem / Sửa / Xoá / Bật / Tắt / Khởi động lại các tunnel (`Quick Tunnel *.trycloudflare.com`, `Zero Trust --token`, `Named Tunnel`) ngay trên giao diện và tự động quét/nhận nuôi/dừng các tiến trình `cloudflared` chạy ngoài luồng trên thiết bị.
- **Bộ nhớ chung & File bàn giao liên AI**: Đồng bộ và biên tập trực tiếp 5 tệp (`MEMORY.md`, `HANDOFF.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`) để Antigravity, Claude Code và Codex phối hợp liền mạch.

## 2. Kiến trúc kỹ thuật cốt lõi
- 2026-11-26 · **Zero-NPM-Runtime Server**: Toàn bộ `server/*.mjs` chỉ dùng module chuẩn của Node.js >= 20.9 (`node:http`, `node:fs`, `node:zlib`, `node:child_process`) để gói `.desktop/app` bên trong Tauri chạy độc lập bằng `.desktop/node/node.exe` mà không cần đóng gói `node_modules` nặng nề.
- 2026-11-26 · **Giải nén `.zip` & `.rar` đa tầng (`server/skill-importer.mjs`)**:
  - `.rar`: Ưu tiên `UnRAR.exe` / `WinRAR.exe` / `unar` / `7z`, dự phòng sang `bsdtar` hệ thống (`C:\Windows\System32\tar.exe` trên Windows 10/11 và `/usr/bin/tar` trên macOS đều tích hợp `libarchive` hỗ trợ RAR4/RAR5).
  - `.zip`: Giải nén thuần Node.js (`zlib.inflateRawSync`) có kiểm tra chống Zip-Slip, dự phòng sang `bsdtar` và `Expand-Archive`.
- 2026-11-26 · **Cloudflare Tunnel Persistence (`server/tunnels.mjs`)**: Lưu cấu hình tại `~/.doto-skill/tunnels.json` và PID/URL đang sống tại `~/.doto-skill/tunnels-runtime.json`; tự động đối chiếu `Win32_Process` (Windows) / `ps` (POSIX) để giữ kết nối xuyên qua các lần tải lại.
- 2026-11-26 · **Tối ưu iOS Safari / iPadOS (`src/styles.css`)**: Ô nhập liệu trên màn hình `<= 767px` đặt `font-size: 16px` để Safari trên iPhone không tự phóng to khung nhìn; thanh điều hướng đáy dùng `env(safe-area-inset-bottom)`.

## 3. Bẫy kỹ thuật cần nhớ
- Trên Windows, PowerShell có thể chặn `npm.ps1` do `ExecutionPolicy` → khi chạy lệnh từ PowerShell hãy gọi `npm.cmd` hoặc `node` trực tiếp; khi spawn CLI (`claude`, `codex`, `agy`) luôn đi qua `resolveCliBinary()` trong `server/cli-resolver.mjs`.
- Khi đóng gói Tauri (`npm.cmd run desktop:build`), đảm bảo `%USERPROFILE%\.cargo\bin` nằm trong `PATH` và luôn chạy `node scripts/build-desktop.mjs --rebuild` sau khi sửa `server/` hoặc `src/` để cập nhật `.desktop/app/`.

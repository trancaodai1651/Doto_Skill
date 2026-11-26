# Tích hợp bên ngoài (kiểm tra theo tài liệu chính thức ngày 2026-11-26)

Mọi lệnh cài chỉ chạy sau khi người dùng xác nhận trong cửa sổ desktop (route `[H]`), log được lọc bí mật, timeout 15 phút. Không lệnh nào yêu cầu `sudo`/Administrator.

## Claude Code (`claude`)
- Cài: Windows `irm https://claude.ai/install.ps1 | iex` · `winget install --id Anthropic.ClaudeCode` · npm `@anthropic-ai/claude-code`; macOS `curl -fsSL https://claude.ai/install.sh | bash` · `brew install --cask claude-code`. Nguồn: https://code.claude.com/docs/en/setup
- Vị trí: `~/.local/bin/claude(.exe)`, npm global.
- Đăng nhập: `claude auth status` → JSON (`loggedIn`, `authMethod`), exit 0 = đã đăng nhập. Nguồn: https://code.claude.com/docs/en/cli-reference
- Skills: `~/.claude/skills/<name>/SKILL.md` (frontmatter tuỳ chọn). Nguồn: https://code.claude.com/docs/en/skills

## OpenAI Codex CLI (`codex`)
- Cài: Windows `irm https://chatgpt.com/codex/install.ps1 | iex` · npm `@openai/codex`; macOS `curl -fsSL https://chatgpt.com/codex/install.sh | sh` · `brew install --cask codex`. Nguồn: https://github.com/openai/codex
- Đăng nhập: `codex login status` — output **in ra stderr**, định dạng không được tài liệu hoá → app chỉ kết luận khi thấy "Logged in"/"Not logged in".
- Skills: `~/.agents/skills/<name>/SKILL.md`, bắt buộc `name` + `description`. Instructions: `AGENTS.md`.

## Google Antigravity CLI (`agy`)
- Cài: Windows `irm https://antigravity.google/cli/install.ps1 | iex` (→ `%LOCALAPPDATA%\agy\bin`); macOS `curl -fsSL https://antigravity.google/cli/install.sh | bash` (→ `~/.local/bin/agy`). Nguồn: https://antigravity.google/docs/install?tab=cli
- Đăng nhập: không có lệnh kiểm tra chính thức → hiển thị "Không rõ".
- Skills: global `~/.gemini/config/skills/<name>/` (bắt buộc `description`); workspace `.agents/skills/`. Rules: `.agents/rules/*.md` **phải có** frontmatter `trigger`. Nguồn: https://antigravity.google/docs/skills · https://antigravity.google/docs/rules

## Cloudflare `cloudflared`
- Cài macOS: `brew install cloudflared`. Windows: Doto Skill tải `cloudflared-windows-amd64.exe` từ https://github.com/cloudflare/cloudflared/releases/latest, kiểm tra SHA-256 (trường `digest` của asset hoặc bảng checksum trong release notes) **và** chữ ký Authenticode (Valid, publisher Cloudflare), lưu vào `<data>/bin/` — không cần quyền admin.
- Quick Tunnel: `cloudflared tunnel --url http://127.0.0.1:<port>` → `https://*.trycloudflare.com`. Chỉ để thử: không SSE, tối đa 200 request đồng thời, không đảm bảo uptime.
- Named/remotely-managed tunnel (production): tạo tunnel trong Cloudflare Zero Trust › Networks › Tunnels, thêm Public Hostname trỏ tới `http://127.0.0.1:<cổng public server>`, sao chép token và dán vào Doto Skill. App chạy `cloudflared tunnel --no-autoupdate run` với token qua biến môi trường `TUNNEL_TOKEN` (không nằm trong argv). Token lưu Keychain/Credential Manager.
- **Khuyến nghị bật Cloudflare Access** (Zero Trust › Access › Applications → Self-hosted, hostname của bạn, policy email/IdP) để có lớp xác thực trước khi request tới máy bạn. Doto Skill vẫn yêu cầu đăng nhập riêng.
- Nguồn: https://developers.cloudflare.com/tunnel/downloads/ · run parameters `TUNNEL_TOKEN`.

## GitHub
- Public repo: tải tarball qua REST API (read-only, không cần đăng nhập; giới hạn 60 request/giờ khi chưa đăng nhập).
- Private repo:
  - **gh CLI** (đã hoạt động): người dùng xác nhận → app chạy `gh auth token`, kiểm tra qua `GET /user`, lưu token vào OS vault.
  - **OAuth Device Flow**: cần OAuth App của chủ sở hữu (bật "Device Flow"), đặt `DOTO_GITHUB_CLIENT_ID`. Scope yêu cầu: `repo read:user`.
- Token chỉ nằm trong header `Authorization`, không bao giờ trong URL, log hay Git remote. Đăng xuất xoá khỏi vault; thu hồi hẳn tại GitHub › Settings › Applications.

## FFmpeg
- Không đóng gói kèm. Dò theo: đường dẫn cấu hình → PATH → WinGet Links / Scoop / Chocolatey / `C:\ffmpeg\bin` (Windows) → `/opt/homebrew/bin`, `/usr/local/bin` (macOS).
- `ffmpeg -version` được phân tích cờ `--enable-gpl`, `--enable-nonfree`, `--enable-version3` để UI cảnh báo giấy phép nếu người dùng định phân phối lại output/binary.

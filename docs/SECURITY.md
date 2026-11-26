# Mô hình bảo mật — Doto Skill

| Mối đe doạ | Biện pháp | Kiểm chứng |
| --- | --- | --- |
| Thực thi lệnh shell / binary tuỳ ý từ trình duyệt | Không có API chạy shell hay đọc file tuỳ ý; CLI chỉ chạy theo `methodId` allowlist biên dịch cứng có xác nhận; `ffmpegPath`/`ffprobePath` bắt buộc tên đúng + xác minh `-version` thật trước khi lưu; Viewer bị chặn `403` khỏi mọi route quản trị | `api_security.rs::auth_csrf_rbac_and_listener_isolation` |
| Chiếm quyền thiết lập lần đầu qua mạng | `/api/auth/desktop` chỉ có trên listener Control (loopback); `/api/auth/setup` chỉ cho phép trực tiếp từ loopback khi chưa có mật khẩu Owner, sau đó khoá vĩnh viễn (`409`); LAN/Tunnel không thể bật khi chưa đặt mật khẩu Owner | cùng test |
| Đánh cắp cookie / CSRF chéo miền | Same-origin dùng cookie tên riêng (`doto_ctl`/`doto_session`), `SameSite=Strict`, `x-csrf-token` và kiểm tra `Origin == Host`. Cross-origin (bản Web độc lập kết nối tới server bản App) dùng `Authorization: Bearer` **không** bật `Access-Control-Allow-Credentials`, kèm vé ngắn hạn `POST /api/auth/ticket` chỉ cho GET media/QR/SSE | test CSRF/Origin → 403 + test CORS Bearer/ticket |
| Brute force | Rate limit 10/5 phút/IP cho login, pair, desktop; Argon2id; cân bằng thời gian khi user không tồn tại | test 429 |
| IDOR / dò tài nguyên | UUID ngẫu nhiên; kiểm tra quyền từng video; không quyền → 404 | test viewer ↔ video nguồn private |
| Path traversal | API chỉ nhận id; đường dẫn đĩa không bao giờ trả về; scanner/skill/tar chặn `..`, symlink, absolute | test traversal + unit test tar/scanner |
| Tar/zip bomb, repo khổng lồ | Giới hạn 200 MB nén, 500 MB giải nén, 50k entry, 2k file/skill, 512 KB/SKILL.md | unit test |
| Skill độc hại | Không tự chạy script; cảnh báo `hasScripts`, `hooks`, `allowed-tools`; cài cần xác nhận + cho phép script riêng | `skills/tests/service.rs` |
| Lộ token | OS vault; header Authorization; lọc bí mật trong log/audit/job log; token tunnel qua env, không argv; secrets-scan trong CI | unit test redact + test audit không chứa token |
| Lộ máy ra Internet ngoài ý muốn | Mặc định Off; LAN cần xác nhận + mật khẩu Owner; cảnh báo IP public; không UPnP; IP allowlist | demo + unit test |
| Nghẽn host | Giới hạn luồng stream theo profile, băng thông tuỳ chọn, giới hạn body 2 MB, scheduler có backpressure | unit test admission |

Báo lỗi bảo mật: mở issue riêng tư (Security advisory) trên GitHub repo, không đăng công khai chi tiết khai thác.

# Mô hình bảo mật — Doto Skill

| Mối đe doạ | Biện pháp | Kiểm chứng |
| --- | --- | --- |
| Trình duyệt từ xa điều khiển máy host (chạy CLI, đọc ổ đĩa) | Route `[H]` chỉ đăng ký trên listener Control (127.0.0.1); phiên `hostControl` chỉ từ token desktop một lần; listener Control từ chối peer không phải loopback | `api_security.rs::auth_csrf_rbac_and_listener_isolation` |
| Đánh cắp cookie giữa 2 listener cùng host | Tên cookie khác nhau; `hostControl` bị gỡ nếu không phải listener Control | cùng test (cookie Control → 401 trên Public) |
| CSRF | SameSite=Strict, header `x-csrf-token` theo phiên, kiểm tra Origin = Host | test CSRF/Origin → 403 |
| Brute force | Rate limit 10/5 phút/IP cho login, pair, desktop; Argon2id; cân bằng thời gian khi user không tồn tại | test 429 |
| IDOR / dò tài nguyên | UUID ngẫu nhiên; kiểm tra quyền từng video; không quyền → 404 | test viewer ↔ video nguồn private |
| Path traversal | API chỉ nhận id; đường dẫn đĩa không bao giờ trả về; scanner/skill/tar chặn `..`, symlink, absolute | test traversal + unit test tar/scanner |
| Tar/zip bomb, repo khổng lồ | Giới hạn 200 MB nén, 500 MB giải nén, 50k entry, 2k file/skill, 512 KB/SKILL.md | unit test |
| Skill độc hại | Không tự chạy script; cảnh báo `hasScripts`, `hooks`, `allowed-tools`; cài cần xác nhận + cho phép script riêng | `skills/tests/service.rs` |
| Lộ token | OS vault; header Authorization; lọc bí mật trong log/audit/job log; token tunnel qua env, không argv; secrets-scan trong CI | unit test redact + test audit không chứa token |
| Lộ máy ra Internet ngoài ý muốn | Mặc định Off; LAN cần xác nhận + mật khẩu Owner; cảnh báo IP public; không UPnP; IP allowlist | demo + unit test |
| Nghẽn host | Giới hạn luồng stream theo profile, băng thông tuỳ chọn, giới hạn body 2 MB, scheduler có backpressure | unit test admission |

Báo lỗi bảo mật: mở issue riêng tư (Security advisory) trên GitHub repo, không đăng công khai chi tiết khai thác.

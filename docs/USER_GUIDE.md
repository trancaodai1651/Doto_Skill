# Hướng dẫn sử dụng & quản trị — Doto Skill

## 1. Lần chạy đầu
1. Mở ứng dụng → tạo **tài khoản Owner** (tên đăng nhập + mật khẩu ≥ 10 ký tự có chữ và số). Mật khẩu dùng khi đăng nhập từ thiết bị khác.
2. Trình hướng dẫn 8 bước: điều khoản & thư mục dữ liệu → kiểm tra phần cứng → chọn chế độ tài nguyên (mặc định **Cân bằng**) → thư mục video nguồn/xuất (có thể bỏ qua) → Skills (**0 skill**, không cài sẵn) → công cụ AI CLI (tuỳ chọn, mỗi lần cài đều hiện lệnh + nguồn để xác nhận) → máy chủ web (Off/Localhost/LAN) → hoàn tất.

## 2. Dùng bản Web với đầy đủ tính năng như bản App & chia sẻ cho thiết bị khác
- **Chủ sở hữu (Owner) trên bản Web:** khi đăng nhập tài khoản Owner từ trình duyệt bất kỳ (PC khác, laptop, máy tính bảng, điện thoại qua LAN hoặc Cloudflare Tunnel), bạn có **đầy đủ 100% tính năng như trên cửa sổ bản App** — mọi tác vụ dò phần cứng, benchmark/render GPU FFmpeg, quản lý Skills, cài AI CLI đều chạy trực tiếp trên phần cứng máy chủ của bản App. Nếu dùng bản Web độc lập, bạn có thể nhập địa chỉ máy chủ bản App ngay tại ô **Máy chủ bản App** ở màn hình Đăng nhập hoặc Cài đặt.
- **Chia sẻ cho Người xem (Viewer):**
  1. **Máy chủ web & mạng** → Chế độ **Mạng LAN** → chọn **interface mạng riêng** (Wi-Fi/Ethernet nội bộ hoặc VPN như Tailscale). Nếu máy có IP public, đừng chọn "Tất cả (0.0.0.0)".
  2. Lưu → xác nhận rủi ro → **Khởi động**. Windows/macOS có thể hỏi quyền tường lửa: chỉ cho phép mạng **Private**.
  3. **Thiết bị** → tạo **mã ghép nối** (Viewer, hết hạn sau 15 phút) → quét QR bằng điện thoại, hoặc tạo tài khoản Viewer.
  4. Trong **Video**: chọn đoạn nguồn → tạo job render → khi xong đặt hiển thị "Người xem thấy được". Viewer chỉ thấy các video này và không vào được bất kỳ trang quản trị nào.

## 3. Truy cập qua Internet (Cloudflare Tunnel)
- Cài `cloudflared` trong **Công cụ AI**.
- **Quick Tunnel** chỉ để thử (URL tạm `*.trycloudflare.com`, không có cập nhật realtime — giao diện tự chuyển sang polling).
- **Named Tunnel** cho sử dụng lâu dài: xem `docs/INTEGRATIONS.md#cloudflare-cloudflared`. Bật **Cloudflare Access** để thêm lớp xác thực.
- Máy chủ chỉ truy cập được khi máy host bật và không ngủ (sleep).

## 4. Skills
- **Skills › Thêm thư mục**: đường dẫn tuyệt đối; bật "Theo dõi thay đổi" để tự quét lại.
- **Thêm từ GitHub**: URL repo, thư mục con (`/tree/<ref>/<path>`) hoặc link `SKILL.md`; nhánh/tag/commit tuỳ chọn. Repo private: kết nối GitHub trước (gh CLI hoặc Device Flow).
- **Kiểm tra cập nhật** hiển thị danh sách file thay đổi trước khi áp dụng; **Pin** để giữ nguyên commit.
- **Cài cho Claude/Codex/Antigravity**: sao chép vào thư mục skills chính thức của từng công cụ. Skill có script cần tích "tôi đã xem nội dung". Doto Skill không bao giờ tự chạy script.

## 5. Hiệu năng
- **Nhẹ**: 1 job, ¼ số luồng CPU, đo mỗi 5 s. **Cân bằng**: tối đa 3 job, ½ luồng, tạm hoãn khi CPU > 85% hoặc RAM thấp. **Tối đa**: tới 8 job, chừa 1 nhân, đo mỗi 1 s.
- Ghi đè thủ công `maxJobs`, `maxCpuThreads`, `maxRamMb`, `maxGpuJobs` (kẹp theo phần cứng). Áp dụng ngay.
- **Kiểm tra encoder** encode thử 1 giây cho từng encoder phần cứng; "Tự động" dùng GPU tốt nhất đã kiểm chứng, lỗi thì tự quay về libx264.

## 6. Sao lưu & khôi phục
**Cài đặt › Sao lưu & khôi phục cơ sở dữ liệu** tạo bản sao SQLite trực tuyến trong `<thư mục dữ liệu>/backups/` và cho phép khôi phục lại bất kỳ bản sao nào sau khi xác nhận. Token GitHub/tunnel nằm trong Keychain/Credential Manager, không nằm trong bản sao lưu.

## 7. Xử lý sự cố
| Triệu chứng | Cách xử lý |
| --- | --- |
| "cổng … đang được ứng dụng khác sử dụng" | Đổi cổng trong Máy chủ web (mặc định 4380) rồi Khởi động. |
| Điện thoại không mở được URL LAN | Cùng mạng? Interface đúng? Tường lửa cho phép mạng Private? Thử `http://<IP>:<cổng>/api/health`. |
| "cần đặt mật khẩu Owner trước khi mở truy cập LAN" | Tạo tài khoản Owner ở màn hình đầu tiên của cửa sổ desktop. |
| "chưa cấu hình FFmpeg" | Cài FFmpeg hoặc nhập đường dẫn `ffmpeg`/`ffprobe` trong Cài đặt, rồi bấm Kiểm tra encoder. |
| Render GPU lỗi | Job tự chuyển libx264; xem Nhật ký job. Cập nhật driver GPU. |
| "không thấy repo" khi import GitHub | Repo private → kết nối GitHub; kiểm tra quyền đọc của tài khoản. |
| Token GitHub hết hạn | Skills › GitHub › Đăng xuất rồi kết nối lại. |
| Hết dung lượng đĩa | Job bị từ chối trước khi chạy (preflight); giải phóng dung lượng hoặc đổi thư mục xuất. |
| Ứng dụng tắt khi đang render | Job được đưa lại hàng đợi khi mở lại ứng dụng. |
| Không có `cloudflared` | Công cụ AI › Cloudflare › Cài, hoặc cài thủ công rồi Làm mới. |

### macOS Gatekeeper
Bản phát hành chưa được Apple notarize. Lần đầu mở: Finder › chuột phải ứng dụng › **Open** › Open. Hoặc: `xattr -dr com.apple.quarantine "/Applications/Doto Skill.app"`.

### Windows SmartScreen
Installer chưa ký số: chọn **More info › Run anyway**. Đối chiếu SHA-256 với `SHA256SUMS.txt` trên trang Release trước khi chạy.

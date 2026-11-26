# MASTER PROMPT — Doto Skill: Desktop + Local Web Server + AI CLI + Video Library

**Dành cho Claude Code / Claude Opus 5.5.** Ngôn ngữ giao tiếp và tài liệu hướng dẫn: tiếng Việt. Tên repo đích: `https://github.com/trancaodai1651/Doto_Skill.git`. Ngày yêu cầu: 2026-11-26.

## 0. Vai trò, mục tiêu, nguyên tắc thực thi

Bạn là Principal Software Architect + Senior Full-stack Engineer + Rust/Tauri Engineer + DevOps/Release Engineer + Security Engineer. **Không chỉ viết kế hoạch, mockup hay ví dụ; phải thực sự tạo, chạy, kiểm thử, đóng gói và bàn giao mã nguồn hoàn chỉnh theo từng mốc kiểm chứng.**

Xây dựng sản phẩm **Doto Skill**, một ứng dụng cài đặt trên **Windows và macOS**, đồng thời là **máy chủ web cục bộ (self-hosted)** để thiết bị khác (PC, tablet, điện thoại) truy cập giao diện quản trị và thư viện video hoàn thiện qua trình duyệt. Ứng dụng tích hợp quản lý nhiều nguồn AI Skills, trình cài đặt tùy chọn cho **Google Antigravity CLI, OpenAI Codex CLI, Claude Code CLI và Cloudflare Tunnel**, nhận diện cấu hình phần cứng máy host, có ba profile tài nguyên **Nhẹ / Cân bằng / Tối đa**, hỗ trợ xử lý/xuất video bằng pipeline thực tế và có bộ nhớ dự án chung để ba agent AI tiếp tục cùng một dự án.

**Repository đã được kiểm tra lúc khởi tạo yêu cầu: `trancaodai1651/Doto_Skill`, nhánh `main`, là repo public nhưng hiện đang trống.** Hãy tự kiểm tra lại lúc bắt đầu; nếu vẫn trống thì khởi tạo dự án mới hoàn toàn. Nếu đã có code thì khảo sát và giữ lại phần có giá trị, không ghi đè phá hoại. Không tự nhận đã chạy/build/test/push/release khi chưa có bằng chứng lệnh và artifact.

Không ngừng ở pha phân tích. Tự chia công việc thành giai đoạn, triển khai liên tiếp trong giới hạn phiên làm việc; nếu giới hạn môi trường hoặc quyền truy cập cản trở thì ghi rõ blocker, phần đã hoàn thành và lệnh tiếp tục trong handoff. Không lặp lại câu hỏi đã đủ dữ kiện. Chỉ yêu cầu người dùng thao tác khi cần đăng nhập, cấp quyền thiết bị/OS, xác thực tài khoản, cài chứng chỉ ký mã hoặc hành động không thể thay mặt thực hiện an toàn.

## 1. Kiến trúc nền tảng và sản phẩm

Chọn kiến trúc **monorepo** có biên giới module rõ ràng, thiên về Rust để tối ưu tài nguyên:

- **Desktop:** Tauri v2 + Rust + React + TypeScript + Vite + Tailwind, native Windows và macOS Intel/Apple Silicon.
- **Web client:** React + TypeScript, giao diện responsive/PWA-installable ở mức phù hợp; cùng design system với desktop, nhưng **không** để web browser trực tiếp chạy lệnh shell/CLI hay truy cập filesystem host.
- **Web/API server:** Rust + Tokio + Axum (hoặc phương án Rust tương đương có giải thích), chạy như thành phần/sidecar của app desktop; có chế độ khởi động cùng hệ điều hành và chạy nền/tray sau khi đóng cửa sổ nếu người dùng chọn. App desktop điều khiển bật/tắt/restart server.
- **State/data:** SQLite + migrations, transaction, backup/restore; có các bảng phù hợp cho app settings, host hardware, CLI tools, skill sources, imported skills, GitHub connections, server config, authorized devices, sessions, video assets, jobs, exports, audit events, handoffs.
- **Media engine:** FFmpeg/ffprobe + Rust orchestrator + queue, không mô phỏng tiến độ; cần cách cấu hình đường dẫn FFmpeg, xác minh binary và lưu ý giấy phép khi phân phối.
- **Realtime:** WebSocket hoặc SSE để hiển thị CPU/RAM/GPU/job progress/server status.
- **Security:** OS Keychain trên macOS / Windows Credential Manager hoặc DPAPI phù hợp; tuyệt đối không lưu access token dạng plaintext trong SQLite hay Git.

Hãy đề xuất `apps/desktop`, `apps/web`, `crates/server`, `crates/core`, `crates/system`, `crates/media`, `crates/skills`, `crates/integrations`, `packages/ui`, `docs`, `scripts`, `.github/workflows` hoặc cấu trúc tốt hơn. Tách adapter Antigravity / Codex / Claude / Cloudflare, tránh tightly coupled.

## 2. UX và onboarding sau khi cài app

- Lần mở app đầu tiên hiển thị trình hướng dẫn từng bước: (1) xác nhận điều khoản và vị trí dữ liệu; (2) kiểm tra phần cứng; (3) chọn profile Nhẹ/Cân bằng/Tối đa, mặc định **Cân bằng**; (4) chọn thư mục nguồn video và thư mục output (có thể bỏ qua); (5) cấu hình **Skills**: trạng thái ban đầu **0 skills, không preinstall, không tự tải skills**; (6) chọn cài từng CLI nếu chưa có; (7) chọn cấu hình server web: Off / Localhost / LAN / Cloudflare; (8) kiểm tra/hoàn tất.
- Các tùy chọn cài CLI và cloudflared **không bắt buộc**, không tự chạy khi chưa được người dùng chấp thuận. UI cho biết đã cài hay chưa, đường dẫn, version, trạng thái đăng nhập, cập nhật, sửa lỗi, gỡ liên kết.
- Có dashboard host, quản lý Skills, nguồn GitHub, AI tools, Video/Exports, Jobs, Web server/Network, Thiết bị đã ghép nối, Performance, Cài đặt, Log/audit và Help.
- Khi chưa có skill, hiển thị trạng thái rỗng cùng nút **Thêm thư mục Skills** / **Import GitHub Skills**; không giả dữ liệu demo là skill thật.

## 3. Cài đặt và quản lý các AI CLI

Hỗ trợ độc lập 4 integration:

1. Google Antigravity CLI
2. OpenAI Codex CLI
3. Anthropic Claude Code CLI
4. Cloudflare `cloudflared`

Mỗi integration phải: dò tìm executable ở PATH và các vị trí cài đặt thông dụng; nhận diện version, kiểm tra tương thích Windows/macOS + CPU architecture, hướng dẫn cài **từ nguồn chính thức cập nhật tại lúc triển khai**, có nút cài bằng phương pháp phù hợp OS; xác minh chữ ký/checksum nếu nguồn hỗ trợ; hiển thị log cài và lỗi; xác minh kết quả; hỗ trợ cấp quyền elevated **chỉ khi cần và có xác nhận**; không hardcode các URL/script không còn hợp lệ; không tự nhập hoặc lưu mật khẩu người dùng. Sử dụng đăng nhập chính thức của từng công cụ, không giả lập OAuth hoặc sửa trái phép credential của chúng.

Hãy xây dựng interface dùng chung `CliProvider` và adapter riêng, gồm discover, install, version, auth status (trong phạm vi SDK/CLI chính thức cho phép), configure, diagnostics, invoke với danh sách command cho phép (allowlist) và sandbox/permission controls. Không cho browser client gọi tùy ý lệnh OS. Nếu provider không có API ổn định, dùng subprocess được cô lập, có timeout, log đã lọc bí mật và explicit user consent. Không tuyên bố tích hợp API không tồn tại.

## 4. Hardware detection, GPU và phân bổ sức mạnh

Trên Windows và macOS tự phát hiện: OS/version/architecture; CPU model, logical/physical cores và mức tải; RAM khả dụng; dung lượng/throughput lưu trữ khi có thể đo an toàn; GPU(s), VRAM khi có API chính thức; hỗ trợ tăng tốc encoder thực tế, phiên bản driver/runtime cần thiết; áp suất bộ nhớ/thermal/battery/power mode nếu OS công bố. Không khẳng định thông tin không thể đọc.

GPU hardware-acceleration phải được probe thực tế thay vì dựa vào tên card: NVIDIA NVENC, Intel Quick Sync, AMD AMF trên hệ tương thích, Apple VideoToolbox trên macOS; fallback software encoder khi không có encoder phù hợp. Có benchmark ngắn tùy chọn và cơ chế an toàn tránh nghẽn/OOM.

**Ba profile người dùng chọn và thay đổi không cần restart khi khả thi:**

- **Nhẹ / Eco:** ít worker, giới hạn CPU/RAM/cache/concurrency, giảm background polling, ưu tiên duy trì độ phản hồi hệ thống và pin.
- **Cân bằng / Balanced (mặc định):** giới hạn động theo tải CPU/RAM/GPU/disk và số job; trải nghiệm UI mượt.
- **Tối đa / Max:** tăng worker/encoder sessions đến giới hạn phần cứng và khả năng driver cho phép; vẫn duy trì headroom, thermal protection, an toàn dữ liệu và khả năng hủy job.

Triển khai `ResourceManager`, `HardwareProbe`, `Scheduler`, bounded queues, semaphores, cancellation, priority, backpressure, disk-space preflight. Cho phép tùy chỉnh thủ công `max_jobs`, `max_cpu_threads`, `max_ram_mb`, `max_gpu_jobs`, bandwidth/throttle nếu khả thi; profile không chỉ là thay đổi một label. Hiển thị metric realtime, mục tiêu sử dụng và actual usage.

## 5. Skills: nhiều thư mục và nhiều repository GitHub

**Không có skill mặc định.** Người dùng có thể thêm, bỏ, tạm dừng, scan lại **không giới hạn cứng số thư mục địa phương và GitHub URL** trong phạm vi tài nguyên máy. Cho phép thêm một hoặc nhiều folder, hỗ trợ nested directories, metadata/index, tìm kiếm/lọc, phát hiện trùng skill bằng canonical identifier + source + revision; giữ provenance mỗi skill. Folder có thể watch thay đổi bằng filesystem watcher, debounced và tránh symlink traversal ngoài phạm vi cấp quyền.

GitHub import: hỗ trợ URL repo public/private, URL thư mục con và link tới một file skill nếu hợp lệ; branch/tag/commit/ref, refresh thủ công và tùy chọn tự kiểm tra update, pin commit khi áp dụng, xem diff thay đổi trước khi cập nhật. **Public**: tải/clone read-only không cần login; **private**: đăng nhập GitHub bằng OAuth Device Flow hoặc flow chính thức phù hợp cho desktop, kiểm tra quyền repo, lưu token trong vault OS, refresh/revoke/logout; tùy chọn tích hợp `gh auth` nếu có và được cho phép. Không in token vào log, không ghi token vào Git remote URL, không commit repository private hoặc nội dung có bản quyền lên repo public `Doto_Skill`.

Coi Skills được import là **untrusted input**: parse/validate YAML/Markdown/manifest, bảo vệ path traversal, symlink escape, zip bomb và command injection; không tự chạy scripts/hooks của skill; xem nội dung/permissions và xin xác nhận rõ trước khi thực thi. Thiết kế cách mapping một skill đã duyệt sang cấu hình/cơ chế của Antigravity, Claude, Codex theo **định dạng hiện được hỗ trợ thật**; không giả định ba công cụ hiểu cùng một format. Quản lý enable/disable từng provider và workspace; không tự động đưa toàn bộ private skill vào prompt context khi không cần.

## 6. Desktop vừa là web server, mở port local và Cloudflare Tunnel

- Desktop có điều khiển **Start / Stop / Restart Web Server**, trạng thái listening, host/port, PID/health, log, số kết nối đang hoạt động và QR code truy cập.
- Mặc định server `127.0.0.1` + cổng khả dụng có thể cấu hình; **không public internet hoặc mở firewall mặc định**.
- Người dùng chọn **LAN** thì mới bind `0.0.0.0` hoặc interface được chọn; hiển thị URL dạng `http://<LAN_IP>:<PORT>`, hướng dẫn/cấp quyền firewall đúng phạm vi, tùy chọn mDNS nếu phù hợp; không tự ý UPnP/router port forwarding.
- Người dùng chọn **Cloudflare Tunnel** thì detect/install `cloudflared` (nếu chưa cài), thiết lập/ủy quyền tunnel theo hướng dẫn chính thức, liên kết hostname + local service, kiểm tra health và hỗ trợ tự khởi động nếu được bật. Tunnel outbound **không phải** mở cổng inbound trên router. Không lưu token tunnel plaintext hoặc in ra log.
- Có chế độ thử nghiệm URL tạm theo tính năng chính thức khi thích hợp và **chế độ production bằng named tunnel + hostname**, không xem URL tạm là endpoint production ổn định.
- Bật quyền truy cập web phải có authentication, session management, role-based access: **Owner/Admin** có quyền cấu hình, **Viewer** chỉ được xem và tải video theo phân quyền. Cho phép login bằng mật khẩu mạnh/SSO phù hợp, có pairing code/QR **hết hạn**, revoke device, rate limit, CSRF/CORS policy, secure cookies theo HTTPS, audit, IP allowlist khi cần. Khuyến nghị và hỗ trợ **Cloudflare Access** khi xuất bản qua internet. Không expose endpoints điều khiển terminal/CLI, token, private Skills hay filesystem cho Viewer.
- Server khởi động độc lập cửa sổ UI qua background/tray phù hợp OS, graceful shutdown; không yêu cầu desktop app hiển thị liên tục để khách xem video nếu người dùng bật background server; nếu máy host tắt/sleep thì báo service không truy cập được, không hứa luôn online.

## 7. Video pipeline, web gallery và tải video hoàn thiện

Người dùng có thể quản lý video nguồn và video đã xuất: import từ folder được cho phép, lập chỉ mục metadata/thumbnail/duration/resolution/codec, danh sách trạng thái job, lịch sử kết xuất và lỗi. Triển khai tối thiểu **một end-to-end rendering pipeline thực**: nhận input video hợp lệ, cho phép thiết lập trim hoặc ghép các đoạn nguồn, xuất MP4 H.264/AAC theo cấu hình, hardware encode khi hỗ trợ; chạy job asynchronous với tiến độ/ETA dựa trên process thật, cancel/retry và file output atomic (không công bố video chưa render xong). Các khả năng AI editing phức tạp để sau nhưng kiến trúc phải mở rộng được theo skill/agent; không hứa đã có auto-edit nếu chưa có implementation.

Web client desktop/tablet/mobile: đăng nhập, responsive grid/list, thumbnail và thông tin video, HTML5 video player có support **HTTP Range/206 Partial Content** để seek, progress/thông báo video mới, bộ lọc, tải về file gốc được cho phép. Download lớn hỗ trợ stream, Content-Disposition, kiểm soát quyền trên từng resource, chặn IDOR và path traversal, không expose đường dẫn disk thật. Có thể share video qua link hạn chế thời gian và revoke nếu được bật, nhưng mặc định private. Server kiểm soát bandwidth/concurrency để không làm nghẽn host.

**Tiêu chí demo E2E:** bật app, bật server LAN, import 1 video hợp lệ, render được output có thể phát, một thiết bị ở cùng LAN đăng nhập quyền Viewer, xem video, seek video và tải file xuống thành công. Có E2E hoặc checklist kiểm chứng thực tế nếu hạ tầng không hỗ trợ test máy khác.

## 8. Bộ nhớ chung và bàn giao giữa ba agent

Tạo nguồn sự thật chung, version-controlled, **không gắn với context tạm của một CLI**:

- `docs/shared/PROJECT_MEMORY.md`: kiến trúc hiện tại, quyết định đã chốt, mô-đun, setup, quy tắc, giả định, hạn chế, mục tiêu.
- `docs/shared/ROADMAP.md`: các phase/mốc, dependency, tiêu chí hoàn thành và trạng thái.
- `docs/shared/DECISIONS.md`: ADR/decision log có ngày và lý do.
- `docs/shared/TASK_BOARD.md`: pending/in progress/done/blocked, owner agent, liên kết commit/PR, next action.
- `docs/shared/CHANGELOG_DEV.md`: thay đổi đã kiểm thử và command/results có thể tái lập.
- `docs/agents/HANDOFF_ANTIGRAVITY.md`, `docs/agents/HANDOFF_CLAUDE.md`, `docs/agents/HANDOFF_CODEX.md`: nội dung bàn giao cụ thể cho mỗi agent, gồm current repo SHA, vừa làm gì, file sửa, tests pass/fail, blocker, việc tiếp theo, cách chạy tiếp.
- `CLAUDE.md` và `AGENTS.md`: chỉ dẫn phù hợp cho Claude Code và Codex, cùng đọc nguồn sự thật chung; tài liệu Antigravity/rules theo **định dạng chính thức hiện hành** và có liên kết tới nguồn sự thật chung.
- `scripts/update-handoff.*` hoặc CLI task để cập nhật đồng bộ handoff sau **mỗi mốc làm việc, mỗi thay đổi quan trọng và trước khi kết thúc phiên**, không tạo trạng thái giả hoặc tự đánh dấu xong khi chưa đạt acceptance.

Quy tắc bắt buộc: **mỗi agent trước khi sửa code phải đọc PROJECT_MEMORY, ROADMAP, TASK_BOARD và handoff của mình; sau khi làm phải cập nhật memory, quyết định, task board và cả 3 handoff.** Chỉ lưu tóm tắt kỹ thuật; không ghi private prompts, access tokens, mật khẩu, secrets, nội dung private repo người dùng.

## 9. Giao diện và chất lượng sản phẩm

Thiết kế giao diện desktop/web thống nhất, hiện đại, dễ dùng, hỗ trợ dark/light, responsive mobile/tablet, tiếng Việt mặc định, chuẩn accessibility cơ bản, empty/loading/error states, inline validation và thông báo rõ ràng. Các màn hình phải đọc dữ liệu thật từ API/state, không để button không hoạt động hoặc fake status. Bảo đảm bất kỳ tác vụ có rủi ro (cài phần mềm, import script, expose LAN/tunnel, xóa video, đăng xuất, đổi thư mục root) đều có xác nhận phù hợp.

## 10. Kế hoạch triển khai bắt buộc

Làm theo các milestone có thể build và demo, không bỏ qua phần nền tảng:

**P0 — Repo & foundation:** clone/khởi tạo main đúng nguồn, thiết lập monorepo, toolchain, code style, CI, SQLite migrations, design system, cấu hình, test skeleton và README chạy máy dev.

**P1 — Desktop/Server/Web:** khởi chạy desktop và Axum server thực, browser responsive, auth/session, start/stop server, LAN mode, healthcheck, firewall guidance, tray/background.

**P2 — Hardware & Resource:** probe phần cứng thật, 3 profile ảnh hưởng scheduler/job thật, metrics dashboard, hardware encoder capability probes.

**P3 — CLI & Skills:** onboarding, detect/install từng CLI + cloudflared, many-folder management, public/private GitHub imports, OS secure credentials, skill validation, agent-specific mapping.

**P4 — Video E2E:** import, FFprobe, thumbnail, render pipeline, queue, H.264 MP4, library, mobile web playback/range/download, RBAC/E2E.

**P5 — Cloudflare & hardening:** cloudflared manager, named tunnel/hostname, integration Cloudflare Access guidance, pairing/revoke, audit, secrets scan, perf/load regression.

**P6 — Release & handoff:** packaging Windows + macOS, GitHub Actions, README, admin/user guide, troubleshooting, release notes, artifact/checksum, 3 agent handoff, push GitHub.

Ở mỗi P: (1) code thật, (2) tests/lint/build, (3) demo acceptance, (4) commit, (5) cập nhật shared memory + 3 handoff rồi mới chuyển pha. Nếu một OS không có runner local thì dùng GitHub-hosted CI runners và chỉ nói verified khi workflow xanh; tránh giả vờ build native macOS từ Windows.

## 11. Kiểm thử, bảo mật và yêu cầu nghiệm thu

Bắt buộc có unit/integration tests cho settings/migrations/resource profiles/skill scanning/GitHub URL parser/auth/download authorization; UI tests Playwright cho onboarding, quản lý sources và web video gallery; tests API security (unauthorized, viewer forbidden admin, path traversal, token handling, range requests, invalid media); sample small media được tạo hợp pháp cho CI và kiểm thử render qua FFmpeg thật trên runner phù hợp. Test xử lý mất mạng, private repo token hết hạn, thiếu `cloudflared`, trùng cổng, host sleep, hết dung lượng, lỗi encode, app restart khi còn jobs.

Có scripts tái lập cho `format`, `lint`, `typecheck`, `unit-test`, `integration-test`, `build`, `release-check`. Không commit `.env`, token, keys, private skill files, media cá nhân hoặc binaries có giấy phép không rõ. Cấu hình GitHub Actions quyền tối thiểu; artifacts chỉ xuất sau khi test phù hợp. Sử dụng dependency locking và kiểm tra licenses (đặc biệt FFmpeg) trước khi đóng gói.

**Definition of Done:** chạy native Windows và macOS (Intel/Apple Silicon hoặc cung cấp binary riêng có ghi rõ); desktop bật/tắt web server thực; server truy cập LAN khi bật; tunnel hoạt động khi được cấu hình hợp lệ; CLI management kiểm tra/cài với sự đồng ý; skills ban đầu trống; multi-folder + multi-GitHub; GitHub private auth an toàn; profile đổi được throughput thực; video render thật + web xem/tải có kiểm soát quyền; CI xanh; installer/assets + checksum + tài liệu + handoff được commit/push.

## 12. Git workflow và xuất bản app

GitHub remote: `https://github.com/trancaodai1651/Doto_Skill.git`; branch mặc định `main`. Trước tiên kiểm tra `git remote -v`, branch, quyền push, protection rules và trạng thái working tree. Không `git push --force`, không xóa lịch sử, không overwrite công việc hiện tại. Phát triển trên branch `feat/initial-platform` hoặc branch phù hợp rồi tạo PR vào `main` nếu quyền/quy tắc repo cho phép. Nếu repo trống chưa có initial commit, khởi tạo an toàn và push commit nền tảng theo đúng quyền. Không tự merge khi yêu cầu quyền review; nếu được phép và kiểm tra đầy đủ thì thực hiện theo cấu hình repo.

Tạo workflow **CI** cho Windows và macOS, workflow **Release** kích hoạt theo version tag (ví dụ `v0.1.0`) sau khi CI thành công, build và upload GitHub Release assets (Windows `.msi`/`.exe` phù hợp, macOS `.dmg`/`.app` hoặc `.zip`, x64/arm64/universal khi toolchain khả thi). Ghi checksum SHA-256, system requirements, hướng dẫn cài, release notes. Nếu thiếu Apple Developer signing/notarization hoặc secrets, tạo pipeline/guide đầy đủ, phát hành unsigned artifacts kèm lưu ý Gatekeeper khi được phép; **không tuyên bố đã notarize hoặc ký số khi chưa thực hiện**. Không tạo tag/release với binary giả hay chỉ là source code nếu mục tiêu là binary chạy được.

**Khi đạt release acceptance và có GitHub auth/permissions:** cập nhật cả ba handoff và shared memory, commit tất cả thay đổi an toàn, push branch/PR/main theo workflow đã chốt, tạo tag, chạy release workflow, kiểm tra artifacts thực tế và đưa URL Release + commit SHA + build/test outcomes. Nếu CI, token, OS toolchain hoặc quyền hạn ngăn release, ghi blocker chính xác cùng lệnh giải quyết; không giấu lỗi và không đánh dấu hoàn thành.

## 13. Báo cáo bắt buộc khi hoàn thành mỗi milestone và khi kết thúc

Trả lời bằng tiếng Việt, báo cáo ngắn nhưng có bằng chứng:

1. Milestone đã hoàn thành; chức năng nào **thực sự dùng được**, chức năng nào còn pending.
2. Danh sách file/module cốt lõi, quyết định kiến trúc mới.
3. Lệnh đã chạy và kết quả test/build/CI; lỗi còn tồn tại và cách tái hiện.
4. Git branch, commit SHA, link PR và link release/artifact nếu **đã được tạo thật**.
5. Trạng thái `PROJECT_MEMORY.md`, `TASK_BOARD.md` và 3 file `HANDOFF_*` đã cập nhật.
6. **NEXT_ACTION** cụ thể đủ để Antigravity/Codex/Claude tiếp tục ngay mà không phân tích lại từ đầu.

**Bắt đầu ngay:** kiểm tra môi trường + repository/permissions, ghi kiến trúc và P0 plan ngắn, sau đó triển khai P0 bằng code thật và tự tiếp tục qua các pha có thể thực hiện. Không chỉ trả lời bằng một bản đặc tả. Ưu tiên một vertical slice có thể chạy/kiểm chứng hơn nhiều tính năng trình diễn không hoạt động.

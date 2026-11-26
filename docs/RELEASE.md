# Quy trình phát hành

1. Cập nhật version đồng bộ: `Cargo.toml` (`workspace.package.version`), `apps/desktop/src-tauri/tauri.conf.json`, `package.json`, `apps/web/package.json`.
2. Viết `docs/release/RELEASE_NOTES_vX.Y.Z.md` (dựa trên `RELEASE_NOTES_TEMPLATE.md`).
3. `npm run release-check` xanh trên máy dev; PR vào `main`, CI xanh trên Windows + macOS.
4. Tag: `git tag -a vX.Y.Z -m "Doto Skill vX.Y.Z" && git push origin vX.Y.Z`.
5. Workflow **Release** chạy lại CI (`workflow_call`), build Windows (NSIS `.exe` + `.msi`) và macOS universal (`.dmg` + `.app.zip`, kiểm tra `lipo` x86_64+arm64), tạo `SHA256SUMS.txt` và GitHub Release.
6. Tải asset, đối chiếu checksum, cài thử.

## Ký mã (chưa cấu hình)
- macOS: thêm secrets `APPLE_CERTIFICATE` (p12 base64), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`. Workflow chỉ bật ký + notarize khi các secrets có mặt.
- Windows: cấu hình `bundle.windows.certificateThumbprint`/`signCommand` trong `tauri.conf.json` khi có chứng chỉ.
- Khi chưa ký: release notes phải ghi rõ "unsigned", hướng dẫn Gatekeeper/SmartScreen. **Không** tuyên bố đã ký/notarize.

## Hệ thống yêu cầu
Windows 10 1809+/11 x64 (WebView2 — có sẵn trên Win 11, installer NSIS tự tải nếu thiếu) · macOS 11+ Intel/Apple Silicon · FFmpeg + ffprobe (cài riêng).

import { spawn, type ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const webDist = path.resolve(__dirname, '..', 'dist');

function resolveServerBin(): string {
  if (process.env.DOTO_SERVER_BIN) return process.env.DOTO_SERVER_BIN;
  const exe = process.platform === 'win32' ? 'doto-server.exe' : 'doto-server';
  const debugBin = path.join(repoRoot, 'target', 'debug', exe);
  const releaseBin = path.join(repoRoot, 'target', 'release', exe);
  if (fs.existsSync(debugBin)) return debugBin;
  if (fs.existsSync(releaseBin)) return releaseBin;
  throw new Error(`Không tìm thấy binary doto-server tại ${debugBin} hoặc ${releaseBin}`);
}

function getFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address();
      const port = typeof addr === 'object' && addr ? addr.port : 0;
      srv.close(() => resolve(port));
    });
    srv.on('error', reject);
  });
}

async function waitForFile(filePath: string, timeoutMs = 20_000): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (fs.existsSync(filePath)) {
      const txt = fs.readFileSync(filePath, 'utf8').trim();
      if (txt.startsWith('http://')) return txt;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(`Quá thời gian chờ token-file từ doto-server: ${filePath}`);
}

/** Tạo tệp ZIP tối giản (Store) chứa SKILL.md ngay trong bộ nhớ mà không cần thư viện ngoài. */
function makeMinimalSkillZip(entryName: string, content: string): Buffer {
  const data = Buffer.from(content, 'utf8');
  const name = Buffer.from(entryName, 'utf8');
  // CRC-32 IEEE
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  crc = (crc ^ 0xffffffff) >>> 0;

  const local = Buffer.alloc(30 + name.length + data.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(data.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(name.length, 26);
  name.copy(local, 30);
  data.copy(local, 30 + name.length);

  const central = Buffer.alloc(46 + name.length);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(data.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(name.length, 28);
  central.writeUInt32LE(0, 42);
  name.copy(central, 46);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(central.length, 12);
  eocd.writeUInt32LE(local.length, 16);

  return Buffer.concat([local, central, eocd]);
}

test.describe('Real doto-server E2E (Desktop App + Web Owner parity + Viewer RBAC)', () => {
  let tmpDir = '';
  let proc: ChildProcess | null = null;
  let desktopBootstrapUrl = '';

  test.beforeAll(async () => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'doto-e2e-real-'));
    const dataDir = path.join(tmpDir, 'data');
    const tokenFile = path.join(tmpDir, 'desktop-url.txt');
    const bin = resolveServerBin();

    proc = spawn(bin, ['--data-dir', dataDir, '--control-port', '0', '--web-root', webDist, '--token-file', tokenFile], {
      cwd: tmpDir,
      env: { ...process.env, DOTO_SECRETS: 'memory', RUST_LOG: 'warn' },
      stdio: 'ignore',
      windowsHide: true,
    });
    desktopBootstrapUrl = await waitForFile(tokenFile);
  });

  test.afterAll(async () => {
    if (proc && !proc.killed) {
      proc.kill();
    }
    if (tmpDir) {
      try {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      } catch {
        /* ignore Windows file lock delay */
      }
    }
  });

  test('full lifecycle on real Rust server: setup, onboarding, zip skill import, memory sync, public web owner & viewer pairing', async ({
    browser,
  }) => {
    const publicPort = await getFreePort();

    // 1. Cửa sổ Desktop (Control listener + one-time hash token)
    const desktopCtx = await browser.newContext({ locale: 'vi-VN' });
    const desktopPage = await desktopCtx.newPage();
    await desktopPage.goto(desktopBootstrapUrl);

    // Thiết lập tài khoản Owner lần đầu
    await expect(desktopPage.getByRole('heading', { name: 'Thiết lập lần đầu' })).toBeVisible();
    await desktopPage.getByLabel('Tên đăng nhập').fill('owner1');
    await desktopPage.getByLabel('Mật khẩu', { exact: true }).fill('StrongPass2026');
    await desktopPage.getByLabel('Nhập lại mật khẩu').fill('StrongPass2026');
    await desktopPage.getByRole('button', { name: 'Tạo tài khoản' }).click();

    // Hoàn tất Onboarding 8 bước trên backend thật
    await expect(desktopPage).toHaveURL(/\/onboarding$/);
    await desktopPage.getByLabel('Tôi đã đọc và đồng ý tiếp tục').check();
    const nextBtn = desktopPage.getByRole('button', { name: 'Tiếp tục' });
    for (let i = 0; i < 7; i++) {
      await nextBtn.click();
    }
    await desktopPage.getByRole('button', { name: 'Hoàn tất thiết lập' }).click();
    await expect(desktopPage.getByRole('heading', { level: 1, name: 'Tổng quan' })).toBeVisible();

    // Bật Public Web Server (localhost:<publicPort>)
    await desktopPage.goto(`${new URL(desktopBootstrapUrl).origin}/server`);
    await desktopPage.getByLabel('Chế độ').selectOption('localhost');
    await desktopPage.getByLabel('Cổng').fill(String(publicPort));
    await desktopPage.getByRole('button', { name: 'Lưu', exact: true }).click();
    await desktopPage.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
    await desktopPage.getByRole('button', { name: 'Khởi động', exact: true }).click();
    await expect(desktopPage.getByText(`http://127.0.0.1:${publicPort}`)).toBeVisible();

    // Tạo mã ghép nối Viewer từ Desktop
    await desktopPage.goto(`${new URL(desktopBootstrapUrl).origin}/devices`);
    await desktopPage.getByRole('button', { name: 'Tạo mã ghép nối' }).click();
    const pairCode = (await desktopPage.locator('[data-testid="pairing-code"], code').filter({ hasText: /^[A-Z0-9]{8}$/ }).first().textContent())?.trim() ?? '';
    expect(pairCode).toMatch(/^[A-Z0-9]{8}$/);

    // 2. Trình duyệt Web Owner kết nối vào http://127.0.0.1:<publicPort> (đầy đủ 100% tính năng trên phần cứng server)
    const publicOrigin = `http://127.0.0.1:${publicPort}`;
    const webOwnerCtx = await browser.newContext({ locale: 'vi-VN' });
    const webOwnerPage = await webOwnerCtx.newPage();
    await webOwnerPage.goto(publicOrigin);
    await webOwnerPage.getByLabel('Tên đăng nhập').fill('owner1');
    await webOwnerPage.getByLabel('Mật khẩu').fill('StrongPass2026');
    await webOwnerPage.getByRole('button', { name: 'Đăng nhập', exact: true }).click();
    await expect(webOwnerPage.getByTestId('remote-owner-banner')).toBeVisible();

    // Import gói Skill .zip thật từ trình duyệt Web Owner
    await webOwnerPage.goto(`${publicOrigin}/skills`);
    await webOwnerPage.getByRole('tab', { name: 'Nguồn' }).click();
    const zipBuf = makeMinimalSkillZip(
      'studio-demo/SKILL.md',
      '---\nname: Studio E2E Skill\ndescription: Kiểm thử E2E trên doto-server thật\n---\n# Nội dung\n',
    );
    await webOwnerPage.locator('input[accept*=".zip"]').setInputFiles({
      name: 'studio-demo.zip',
      mimeType: 'application/zip',
      buffer: zipBuf,
    });
    await webOwnerPage.getByTestId('import-archive-btn').click();
    await webOwnerPage.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
    await webOwnerPage.getByRole('tab', { name: 'Danh sách skill' }).click();
    await expect(webOwnerPage.getByTestId('skills-table')).toContainText('Studio E2E Skill');

    // Ghi bàn giao từ Web Owner trên trang Bộ nhớ & Bàn giao
    await webOwnerPage.goto(`${publicOrigin}/memory`);
    await webOwnerPage.getByLabel('Tóm tắt phiên vừa làm').fill('Kiểm chứng E2E thật thành công');
    await webOwnerPage.getByRole('button', { name: 'Ghi bàn giao' }).click();
    await expect(webOwnerPage.getByText('Kiểm chứng E2E thật thành công')).toBeVisible();

    // Tải ảnh từ thiết bị lên máy chủ qua Web Owner
    await webOwnerPage.goto(`${publicOrigin}/videos`);
    const tinyPng = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=', 'base64');
    await webOwnerPage.getByTestId('mobile-upload-label').locator('input[type="file"]').setInputFiles({
      name: 'app-photos-test.png',
      mimeType: 'image/png',
      buffer: tinyPng,
    });
    await expect(webOwnerPage.getByText(/Media đã tải lên gần đây/)).toBeVisible();

    // 3. Trình duyệt Viewer ghép nối bằng mã 8 ký tự → chỉ thấy Thư viện video, bị chặn trang quản trị
    const viewerCtx = await browser.newContext({ locale: 'vi-VN' });
    const viewerPage = await viewerCtx.newPage();
    await viewerPage.goto(publicOrigin);
    await viewerPage.getByRole('tab', { name: 'Ghép nối thiết bị' }).click();
    await viewerPage.getByLabel('Mã ghép nối').fill(pairCode);
    await viewerPage.getByLabel('Tên thiết bị').fill('iPhone Kiểm Thử');
    await viewerPage.getByRole('button', { name: 'Ghép nối' }).click();
    await expect(viewerPage).toHaveURL(/\/gallery$/);
    await viewerPage.goto(`${publicOrigin}/studio`);
    await expect(viewerPage).toHaveURL(/\/gallery$/);

    await desktopCtx.close();
    await webOwnerCtx.close();
    await viewerCtx.close();
  });
});

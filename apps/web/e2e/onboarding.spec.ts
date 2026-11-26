import { expect, test } from '@playwright/test';
import type { AppSettings, Me, ProfileId, ServerConfig } from '../src/api/types';
import { mockApi, noContent, ok } from './mock-api';
import * as fx from './fixtures';

test('onboarding happy path through all 8 steps (desktop window)', async ({ page }) => {
  let session: Me = fx.anonymous;
  let profile: ProfileId = 'balanced';
  let appSettings: AppSettings = { ...fx.settings };
  let srvConfig: ServerConfig = fx.serverConfig('off');

  const { calls } = await mockApi(page, {
    'POST /api/auth/desktop': () => {
      session = fx.me({ onboardingCompleted: false });
      return noContent();
    },
    'GET /api/auth/me': () => ok(session),
    'GET /api/settings': () => ok(appSettings),
    'PUT /api/settings': (r) => {
      appSettings = r.body as AppSettings;
      return ok(appSettings);
    },
    'GET /api/system/hardware': () => ok(fx.hardware),
    'POST /api/system/hardware/probe': () => ok(fx.hardware),
    'GET /api/system/metrics': () => ok(fx.metrics(profile)),
    'GET /api/jobs': () => ok([]),
    'GET /api/videos': () => ok([]),
    'GET /api/settings/resources': () => ok(fx.resources(profile)),
    'PUT /api/settings/resources': (r) => {
      profile = (r.body as { profile: ProfileId }).profile;
      return ok(fx.resources(profile));
    },
    'GET /api/skill-sources': () => ok([]),
    'GET /api/tools': () => ok(fx.tools),
    'GET /api/server/config': () => ok(srvConfig),
    'PUT /api/server/config': (r) => {
      srvConfig = r.body as ServerConfig;
      return ok(srvConfig);
    },
    'POST /api/server/restart': () => ok(fx.serverStatus(srvConfig.mode)),
    'GET /api/server/status': () => ok(fx.serverStatus(srvConfig.mode)),
    'GET /api/server/firewall-guide': () => ok(fx.firewallGuide),
    'POST /api/onboarding/complete': () => {
      session = { ...session, onboardingCompleted: true };
      return ok({ ok: true });
    },
  });

  // Desktop bootstrap: one-time token in the hash is exchanged then stripped.
  await page.goto('/#desktop-token=one-time-abc');
  await expect(page).toHaveURL(/\/onboarding$/);
  const desktopCall = calls.find((c) => c.path === '/api/auth/desktop');
  expect(desktopCall?.body).toEqual({ token: 'one-time-abc' });
  expect(await page.evaluate(() => location.hash)).toBe('');

  const next = page.getByRole('button', { name: 'Tiếp tục' });
  const heading = page.locator('#onb-step-title');

  // 1. Terms & data dir
  await expect(heading).toHaveText('Điều khoản & vị trí dữ liệu');
  await expect(page.getByText(fx.settings.dataDir)).toBeVisible();
  await expect(next).toBeDisabled();
  await page.getByLabel('Tôi đã đọc và đồng ý tiếp tục').check();
  await next.click();

  // 2. Hardware + probe
  await expect(heading).toHaveText('Kiểm tra phần cứng');
  await expect(page.getByText('Test CPU 8-Core', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Dò lại phần cứng' }).click();
  await expect.poll(() => calls.some((c) => c.path === '/api/system/hardware/probe')).toBe(true);
  await next.click();

  // 3. Profile — Cân bằng is default; switch to Tối đa and see effective limits change
  await expect(heading).toHaveText('Chế độ tài nguyên');
  await expect(page.getByRole('radio', { name: /Cân bằng/ })).toHaveAttribute('aria-checked', 'true');
  const limits = page.getByTestId('limits-table');
  await expect(limits.getByRole('row', { name: /Job đồng thời tối đa/ })).toContainText('2');
  await page.getByRole('radio', { name: /Tối đa/ }).click();
  await expect(page.getByRole('radio', { name: /Tối đa/ })).toHaveAttribute('aria-checked', 'true');
  await expect(limits.getByRole('row', { name: /Job đồng thời tối đa/ })).toContainText('4');
  await next.click();

  // 4. Folders — add a source dir and save via ConfirmDialog
  await expect(heading).toHaveText('Thư mục video');
  await page.getByLabel('Thêm thư mục nguồn').fill('D:\\Videos');
  await page.getByRole('button', { name: 'Thêm', exact: true }).click();
  await page.getByRole('button', { name: 'Lưu', exact: true }).click();
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toContainText('Thay đổi thư mục gốc?');
  await dialog.getByTestId('confirm-dialog-confirm').click();
  await expect(dialog).toBeHidden();
  expect(appSettings.videoSourceDirs).toEqual(['D:\\Videos']);
  await next.click();

  // 5. Skills — explicitly zero, nothing bundled; skip
  await expect(heading).toHaveText('Skills');
  await expect(page.getByText('0 skills — không cài sẵn, không tự tải')).toBeVisible();
  await page.getByRole('button', { name: 'Bỏ qua' }).click();

  // 6. AI CLI — install shows exact command preview; cancel => nothing runs
  await expect(heading).toHaveText('Công cụ AI CLI');
  await page.getByRole('button', { name: 'Cài qua npm' }).click();
  await expect(page.getByTestId('install-command')).toHaveText('npm install -g @anthropic-ai/claude-code');
  await page.getByRole('alertdialog').getByRole('button', { name: 'Huỷ' }).click();
  expect(calls.some((c) => c.path.startsWith('/api/tools/') && c.method === 'POST')).toBe(false);
  await next.click();

  // 7. Server — LAN requires acknowledging the exposure warning
  await expect(heading).toHaveText('Máy chủ web');
  await page.getByRole('radio', { name: /Mạng LAN/ }).check();
  await page.getByRole('button', { name: 'Áp dụng' }).click();
  const lan = page.getByRole('alertdialog');
  await expect(lan).toContainText('Mở truy cập trong mạng LAN?');
  const confirmLan = lan.getByTestId('confirm-dialog-confirm');
  await expect(confirmLan).toBeDisabled();
  await lan.getByLabel('Tôi hiểu rủi ro khi mở truy cập trong mạng LAN').check();
  await confirmLan.click();
  await expect(lan).toBeHidden();
  expect(srvConfig.mode).toBe('lan');
  const putCfg = calls.find((c) => c.method === 'PUT' && c.path === '/api/server/config');
  expect(putCfg?.headers['x-csrf-token']).toBe('csrf-test');
  await next.click();

  // 8. Review & finish
  await expect(heading).toHaveText('Xem lại & hoàn tất');
  await expect(page.getByText('Tối đa', { exact: true })).toBeVisible();
  await expect(page.getByText('D:\\Videos')).toBeVisible();
  await page.getByRole('button', { name: 'Hoàn tất thiết lập' }).click();

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole('heading', { level: 1, name: 'Tổng quan' })).toBeVisible();
  expect(calls.some((c) => c.method === 'POST' && c.path === '/api/onboarding/complete')).toBe(true);
});

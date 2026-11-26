import { expect, test } from '@playwright/test';
import type { Me } from '../src/api/types';
import { mockApi, ok } from './mock-api';
import * as fx from './fixtures';

test('viewer: login, gallery grid, player page with download link, no admin nav', async ({ page }) => {
  let session: Me = fx.anonymous;
  const videos = [fx.video('v1', 'Hướng dẫn CapCut'), fx.video('v2', 'Bản tin sáng', { durationMs: 61_000, width: 1080, height: 1920 })];
  const { calls } = await mockApi(page, {
    'GET /api/auth/me': () => ok(session),
    'POST /api/auth/login': (r) => {
      const b = r.body as { username: string; password: string };
      if (b.username !== 'xem' || b.password !== 'matkhau12345') return { status: 401, json: { error: 'unauthorized', message: 'Sai tên đăng nhập hoặc mật khẩu' } };
      session = fx.me({ user: fx.viewerUser, hostControl: false, csrfToken: 'csrf-viewer' });
      return ok(session);
    },
    'GET /api/videos': () => ok(videos),
    'GET /api/videos/:id': (r) => {
      const v = videos.find((x) => x.id === r.params.id);
      return v ? ok(v) : { status: 404, json: { error: 'not_found', message: 'Không tìm thấy' } };
    },
  });

  await page.goto('/');
  await expect(page.getByRole('tab', { name: 'Đăng nhập' })).toHaveAttribute('aria-selected', 'true');

  // wrong password shows server message
  await page.getByLabel('Tên đăng nhập').fill('xem');
  await page.getByLabel('Mật khẩu').fill('sai');
  await page.getByRole('button', { name: 'Đăng nhập', exact: true }).click();
  await expect(page.getByText('Sai tên đăng nhập hoặc mật khẩu')).toBeVisible();

  await page.getByLabel('Mật khẩu').fill('matkhau12345');
  await page.getByRole('button', { name: 'Đăng nhập', exact: true }).click();

  await expect(page).toHaveURL(/\/gallery$/);
  const grid = page.getByTestId('gallery-grid');
  await expect(grid.getByRole('listitem')).toHaveCount(2);
  await expect(grid).toContainText('Hướng dẫn CapCut');
  await expect(grid).toContainText('Bản tin sáng');
  await expect(grid).toContainText('1920×1080');

  // admin navigation is not rendered for viewers
  await expect(page.getByRole('navigation', { name: 'Điều hướng chính' })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Skills' })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Tổng quan' })).toHaveCount(0);

  // viewers trying an admin URL are bounced back to the gallery
  await page.goto('/skills');
  await expect(page).toHaveURL(/\/gallery$/);

  await page.getByRole('link', { name: /Hướng dẫn CapCut/ }).click();
  await expect(page).toHaveURL(/\/gallery\/v1$/);
  await expect(page.getByRole('heading', { level: 1, name: 'Hướng dẫn CapCut' })).toBeVisible();
  const player = page.getByTestId('video-player');
  await expect(player).toHaveAttribute('src', '/api/videos/v1/stream');
  await expect(player).toHaveAttribute('preload', 'metadata');
  await expect(player).toHaveAttribute('playsinline', '');
  const download = page.getByTestId('download-link');
  await expect(download).toHaveAttribute('href', '/api/videos/v1/download');

  // only viewer-permitted endpoints were touched
  const adminCalls = calls.filter((c) => /^\/api\/(skills|skill-sources|tools|server|settings|system|devices|users|jobs)/.test(c.path));
  expect(adminCalls).toEqual([]);
});

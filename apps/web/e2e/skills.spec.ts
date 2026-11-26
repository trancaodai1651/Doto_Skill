import { expect, test } from '@playwright/test';
import type { SkillSource } from '../src/api/types';
import { mockApi, ok } from './mock-api';
import * as fx from './fixtures';

test('skills: empty state shows "0 skills", adding a folder source updates the list', async ({ page }) => {
  const sources: SkillSource[] = [];
  const { calls } = await mockApi(page, {
    'GET /api/auth/me': () => ok(fx.me()),
    'GET /api/skill-sources': () => ok(sources),
    'GET /api/skills': () => ok([]),
    'POST /api/skill-sources/folder': (r) => {
      const s = fx.folderSource((r.body as { path: string }).path);
      sources.push(s);
      return ok(s);
    },
    'GET /api/system/metrics': () => ok(fx.metrics()),
    'GET /api/jobs': () => ok([]),
    'GET /api/videos': () => ok([]),
  });

  await page.goto('/skills');
  await expect(page.getByRole('heading', { level: 1, name: 'Skills' })).toBeVisible();
  await expect(page.getByText('0 skills — không cài sẵn, không tự tải')).toBeVisible();

  await page.getByRole('tab', { name: 'Nguồn' }).click();
  await expect(page.getByText('Chưa có nguồn skill nào')).toBeVisible();

  const form = page.getByRole('form', { name: 'Thêm thư mục' });
  // relative paths are rejected inline
  await form.getByLabel('Đường dẫn thư mục').fill('skills');
  await form.getByRole('button', { name: 'Thêm thư mục' }).click();
  await expect(form.getByText('Cần đường dẫn tuyệt đối', { exact: false })).toBeVisible();

  await form.getByLabel('Đường dẫn thư mục').fill('D:\\Skills');
  await form.getByRole('button', { name: 'Thêm thư mục' }).click();
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toContainText('Cấp quyền đọc thư mục này?');
  await expect(dialog).toContainText('D:\\Skills');
  await dialog.getByTestId('confirm-dialog-confirm').click();
  await expect(dialog).toBeHidden();

  const post = calls.find((c) => c.method === 'POST' && c.path === '/api/skill-sources/folder');
  expect(post?.body).toEqual({ path: 'D:\\Skills', watch: true });
  expect(post?.headers['x-csrf-token']).toBe('csrf-test');

  const list = page.getByTestId('sources-list');
  await expect(list).toContainText('My Skills');
  await expect(list).toContainText('D:\\Skills');
  await expect(list).toContainText('0 skill');
});

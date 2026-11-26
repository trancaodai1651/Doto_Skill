import { expect, test } from '@playwright/test';
import type { BackupEntry, Me, SkillSource } from '../src/api/types';
import { mockApi, ok } from './mock-api';
import * as fx from './fixtures';

test('web owner has full feature parity with desktop app using host server hardware', async ({ page }) => {
  const session: Me = {
    ...fx.me({ hostControl: false }),
    hostname: 'DOTO-HOST-PC',
  };
  const sources: SkillSource[] = [];
  const backups: BackupEntry[] = [{ file: 'doto-20261126-120000.sqlite', sizeBytes: 65536, createdAt: '2026-11-26T05:00:00Z' }];

  const { calls } = await mockApi(page, {
    'GET /api/auth/me': () => ok(session),
    'GET /api/system/hardware': () => ok(fx.hardware),
    'GET /api/system/metrics': () => ok(fx.metrics()),
    'GET /api/settings/resources': () => ok(fx.resources('balanced')),
    'GET /api/system/ffmpeg': () =>
      ok({
        found: true,
        ffmpegPath: 'C:\\ffmpeg\\bin\\ffmpeg.exe',
        ffprobePath: 'C:\\ffmpeg\\bin\\ffprobe.exe',
        version: '9.0.2',
        licenseFlags: ['gpl'],
        encoders: [
          { name: 'h264_nvenc', kind: 'hardware', vendor: 'nvidia', available: true, working: true, error: null },
          { name: 'libx264', kind: 'software', vendor: 'x264', available: true, working: true, error: null },
        ],
        encodersProbedAt: '2026-11-26T05:00:00Z',
      }),
    'POST /api/system/ffmpeg/benchmark': () =>
      ok({
        ranAt: '2026-11-26T05:01:00Z',
        results: [
          { name: 'h264_nvenc', kind: 'hardware', vendor: 'nvidia', frames: 60, elapsedMs: 120, fps: 500.0 },
          { name: 'libx264', kind: 'software', vendor: 'x264', frames: 60, elapsedMs: 300, fps: 200.0 },
        ],
      }),
    'GET /api/skill-sources': () => ok(sources),
    'GET /api/skills': () => ok([]),
    'POST /api/skill-sources/folder': (r) => {
      const s = fx.folderSource((r.body as { path: string }).path);
      sources.push(s);
      return ok(s);
    },
    'GET /api/settings': () => ok(fx.settings),
    'GET /api/system/backups': () => ok(backups),
    'POST /api/system/restore': (r) => ok({ ok: true, file: (r.body as { file: string }).file }),
    'GET /api/jobs': () => ok([]),
    'GET /api/videos': () => ok([]),
  });

  // 1. Web Owner sees host hardware connection banner & zero desktop-only lockouts
  await page.goto('/skills');
  await expect(page.getByTestId('remote-owner-banner')).toContainText('DOTO-HOST-PC');
  await expect(page.getByTestId('host-only-note')).toHaveCount(0);

  // 2. Web Owner can add a folder skill source on the host machine (with host disk hints)
  await page.getByRole('tab', { name: 'Nguồn' }).click();
  const form = page.getByRole('form', { name: 'Thêm thư mục' });
  await form.getByLabel('Đường dẫn thư mục').fill('D:\\HostSkills');
  await form.getByRole('button', { name: 'Thêm thư mục' }).click();
  await page.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
  await expect(page.getByTestId('sources-list')).toContainText('D:\\HostSkills');

  // 3. Web Owner can run hardware GPU/CPU FFmpeg benchmark on the host server
  await page.goto('/performance');
  await page.getByRole('button', { name: 'Benchmark 720p' }).click();
  const benchTable = page.getByTestId('benchmark-table');
  await expect(benchTable).toContainText('h264_nvenc');
  await expect(benchTable).toContainText('500.0 fps');

  // 4. Web Owner can see server connection card & restore SQLite backup in Settings
  await page.goto('/settings');
  await expect(page.getByTestId('server-connection-box')).toBeVisible();
  await expect(page.getByText('doto-20261126-120000.sqlite')).toBeVisible();
  await page.getByRole('button', { name: 'Khôi phục' }).click();
  await page.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
  expect(calls.some((c) => c.method === 'POST' && c.path === '/api/system/restore')).toBe(true);
});

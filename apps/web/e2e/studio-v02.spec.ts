import { expect, test } from '@playwright/test';
import type { ManagedTunnel, Me, StudioRunDetail } from '../src/api/types';
import { mockApi, ok } from './mock-api';
import * as fx from './fixtures';

test('v0.2 studio: AI Studio multi-session, Memory & Handoff center, Folder Picker & Multi-Tunnel CRUD', async ({ page }) => {
  const session: Me = { ...fx.me({ hostControl: false }), hostname: 'STUDIO-HOST' };
  const runs: StudioRunDetail[] = [];
  const managedTunnels: ManagedTunnel[] = [];
  let memoryContent = '# MEMORY.md\n\nQuy ước ban đầu.';

  await mockApi(page, {
    'GET /api/auth/me': () => ok(session),
    'GET /api/system/metrics': () => ok(fx.metrics()),
    'GET /api/jobs': () => ok([]),
    'GET /api/videos': () => ok([]),
    'GET /api/skills': () => ok([]),
    'GET /api/uploads': () => ok([]),
    'GET /api/studio/catalog': () =>
      ok({
        providers: [
          {
            id: 'antigravity',
            name: 'Google Antigravity CLI',
            vendor: 'Google DeepMind',
            installed: true,
            binaryPath: 'C:\\agy\\agy.exe',
            version: '1.2.0',
            skillPrefix: '/',
            models: [{ id: 'gemini-2.5-pro', name: 'Gemini Pro', badge: null, defaultEffort: 'high', efforts: ['low', 'medium', 'high'], slugPattern: null, desc: '' }],
          },
          {
            id: 'claude',
            name: 'Anthropic Claude Code',
            vendor: 'Anthropic',
            installed: true,
            binaryPath: 'C:\\claude\\claude.exe',
            version: '2.1.0',
            skillPrefix: '/',
            models: [{ id: 'claude-opus-4-6', name: 'Claude Opus 4.6', badge: null, defaultEffort: 'high', efforts: ['low', 'medium', 'high'], slugPattern: null, desc: '' }],
          },
          {
            id: 'codex',
            name: 'OpenAI Codex CLI',
            vendor: 'OpenAI',
            installed: true,
            binaryPath: 'C:\\codex\\codex.cmd',
            version: '0.166.0',
            skillPrefix: '$',
            models: [{ id: 'gpt-5.4', name: 'GPT-5.4 Codex', badge: null, defaultEffort: 'high', efforts: ['low', 'medium', 'high'], slugPattern: null, desc: '' }],
          },
        ],
      }),
    'GET /api/studio/runs': () => ok(runs),
    'POST /api/studio/runs': (r) => {
      const b = r.body as { prompt: string };
      const detail: StudioRunDetail = {
        job: {
          id: 'job-studio-1',
          kind: 'studio',
          status: 'succeeded',
          title: `Studio (3 phiên): ${b.prompt}`,
          progress: 1,
          etaSeconds: null,
          speed: null,
          encoder: '3 phiên AI',
          error: null,
          attempts: 1,
          outputVideoId: null,
          createdAt: '2026-11-27T00:00:00Z',
          startedAt: '2026-11-27T00:00:00Z',
          finishedAt: '2026-11-27T00:00:05Z',
        },
        sessions: [
          {
            id: 's1',
            jobId: 'job-studio-1',
            sessionIndex: 1,
            label: 'ANTIGRAVITY',
            provider: 'antigravity',
            model: '',
            effort: 'high',
            skillId: null,
            status: 'succeeded',
            commandPreview: 'agy -p "..."',
            outputSubdir: 'D:\\Exports\\s1',
            resultText: 'Phương án Antigravity hoàn tất.',
            log: ['[khởi tạo] agy'],
            error: null,
            exitCode: 0,
            outputVideoIds: [],
            startedAt: '2026-11-27T00:00:00Z',
            finishedAt: '2026-11-27T00:00:05Z',
          },
          {
            id: 's2',
            jobId: 'job-studio-1',
            sessionIndex: 2,
            label: 'CLAUDE',
            provider: 'claude',
            model: '',
            effort: 'high',
            skillId: null,
            status: 'succeeded',
            commandPreview: 'claude -p "..."',
            outputSubdir: 'D:\\Exports\\s2',
            resultText: 'Phương án Claude hoàn tất.',
            log: ['[khởi tạo] claude'],
            error: null,
            exitCode: 0,
            outputVideoIds: [],
            startedAt: '2026-11-27T00:00:00Z',
            finishedAt: '2026-11-27T00:00:05Z',
          },
        ],
      };
      runs.unshift(detail);
      return ok(detail);
    },
    'GET /api/memory': () =>
      ok({
        workspaceDir: 'E:\\Workspace',
        isCustomDir: false,
        docs: [
          {
            id: 'PROJECT_MEMORY.md',
            relPath: 'docs/shared/PROJECT_MEMORY.md',
            group: 'shared',
            titleVi: 'Bộ nhớ dự án',
            titleEn: 'Project memory',
            agents: ['antigravity', 'claude', 'codex'],
            exists: true,
            sizeBytes: 120,
            updatedAt: '2026-11-27T00:00:00Z',
            preview: memoryContent,
          },
        ],
        recentHandoffs: [],
      }),
    'GET /api/memory/doc': () => ok({ id: 'PROJECT_MEMORY.md', relPath: 'docs/shared/PROJECT_MEMORY.md', exists: true, content: memoryContent }),
    'PUT /api/memory/doc': (r) => {
      memoryContent = (r.body as { content: string }).content;
      return ok({ ok: true, id: 'PROJECT_MEMORY.md' });
    },
    'GET /api/server/status': () => ok(fx.serverStatus('localhost')),
    'GET /api/server/config': () => ok(fx.serverConfig('localhost')),
    'GET /api/server/firewall-guide': () => ok(fx.firewallGuide),
    'GET /api/tunnel/status': () =>
      ok({
        cloudflaredFound: true,
        cloudflaredPath: 'C:\\cloudflared.exe',
        running: false,
        mode: 'quick',
        publicUrl: null,
        hostname: null,
        hasToken: false,
        autoStart: false,
        startedAt: null,
        lastError: null,
        logTail: [],
      }),
    'GET /api/tunnels': () =>
      ok({
        cloudflaredFound: true,
        cloudflaredPath: 'C:\\cloudflared.exe',
        defaultTargetUrl: 'http://127.0.0.1:4380',
        managed: managedTunnels,
        external: [],
      }),
    'POST /api/tunnels': (r) => {
      const b = r.body as { name: string; kind: 'quick' | 'token' | 'named'; targetUrl: string };
      const m: ManagedTunnel = {
        id: 't1',
        name: b.name,
        kind: b.kind,
        targetUrl: b.targetUrl,
        hostname: 'studio.example.com',
        cfTunnelName: null,
        hasToken: true,
        autoStart: false,
        notes: '',
        running: false,
        connected: false,
        pid: null,
        publicUrl: 'https://studio.example.com',
        startedAt: null,
        lastError: null,
        logTail: [],
        createdAt: '2026-11-27T00:00:00Z',
        updatedAt: '2026-11-27T00:00:00Z',
      };
      managedTunnels.push(m);
      return ok(m);
    },
    'GET /api/settings': () => ok(fx.settings),
    'GET /api/system/hardware': () => ok(fx.hardware),
    'GET /api/system/backups': () => ok([]),
    'GET /api/fs/dirs': () =>
      ok({
        current: 'D:\\Media',
        parent: 'D:\\',
        roots: [{ label: 'Ổ đĩa D:\\', path: 'D:\\' }],
        entries: [{ name: 'Clips2026', path: 'D:\\Media\\Clips2026' }],
      }),
  });

  // 1. AI Studio multi-AI parallel launch
  await page.goto('/studio');
  await page.getByLabel('Chế độ điều phối').selectOption('multi_ai');
  await page.getByLabel('Yêu cầu thực hiện (Prompt)').fill('Tạo bản tin AI 60s');
  await page.getByTestId('studio-launch-btn').click();
  await page.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
  await expect(page.getByTestId('studio-runs-list')).toContainText('Phương án Antigravity hoàn tất.');
  await expect(page.getByTestId('studio-runs-list')).toContainText('Phương án Claude hoàn tất.');

  // 2. Shared Memory & Handoff Center
  await page.goto('/memory');
  await expect(page.getByTestId('memory-docs-list')).toContainText('PROJECT_MEMORY.md');
  await page.getByRole('textbox', { name: 'PROJECT_MEMORY.md' }).fill('# MEMORY.md\n\nĐã cập nhật từ Web Owner.');
  await page.getByTestId('memory-save-btn').click();
  await page.getByRole('alertdialog').getByTestId('confirm-dialog-confirm').click();
  expect(memoryContent).toContain('Đã cập nhật từ Web Owner.');

  // 3. Multi-Tunnel CRUD on Server page
  await page.goto('/server');
  await page.getByRole('button', { name: /Thêm cấu hình Tunnel/ }).click();
  await page.getByLabel('Tên gợi nhớ').fill('Zero Trust Studio');
  await page.getByRole('region', { name: /CRUD/ }).getByRole('button', { name: 'Lưu', exact: true }).click();
  await expect(page.getByTestId('managed-tunnels-list')).toContainText('Zero Trust Studio');

  // 4. Host Folder Picker modal in Settings
  await page.goto('/settings');
  await page.getByRole('button', { name: /Duyệt thư mục/ }).first().click();
  await expect(page.getByTestId('folder-picker-modal')).toContainText('Clips2026');
});

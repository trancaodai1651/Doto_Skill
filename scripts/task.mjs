#!/usr/bin/env node
// Task runner đa nền tảng: `npm run <task>` → node scripts/task.mjs <task>
// format | format-check | lint | typecheck | unit-test | integration-test | e2e | build | release-check | desktop-dev | desktop-build
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const isWin = process.platform === 'win32';

function run(cmd, args, opts = {}) {
  const pretty = `${cmd} ${args.join(' ')}`;
  console.log(`\n▶ ${pretty}${opts.cwd ? `  (cwd: ${path.relative(root, opts.cwd) || '.'})` : ''}`);
  const r = spawnSync(cmd, args, { stdio: 'inherit', cwd: opts.cwd ?? root, shell: isWin, env: { ...process.env, ...(opts.env ?? {}) } });
  if (r.status !== 0) {
    console.error(`✖ thất bại: ${pretty} (mã ${r.status})`);
    process.exit(r.status ?? 1);
  }
}

const npm = 'npm';
const desktopDir = path.join(root, 'apps', 'desktop');
const testEnv = { DOTO_SECRETS: 'memory' };

const tasks = {
  format: () => {
    run('cargo', ['fmt', '--all']);
  },
  'format-check': () => {
    run('cargo', ['fmt', '--all', '--check']);
  },
  lint: () => {
    tasks['web-build']();
    run('cargo', ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']);
    run(npm, ['run', 'lint', '-w', 'apps/web']);
  },
  typecheck: () => {
    run(npm, ['run', 'typecheck', '-w', 'apps/web']);
    tasks['web-build']();
    run('cargo', ['check', '--workspace', '--all-targets']);
  },
  'web-build': () => {
    run(npm, ['run', 'build', '-w', 'apps/web']);
  },
  'unit-test': () => {
    run(npm, ['run', 'test', '-w', 'apps/web']);
    tasks['web-build']();
    run('cargo', ['test', '--workspace', '--lib', '--bins'], { env: testEnv });
  },
  'integration-test': () => {
    tasks['web-build']();
    // cần ffmpeg/ffprobe trên PATH; CI đặt DOTO_REQUIRE_FFMPEG=1 để bắt buộc
    run('cargo', ['test', '--workspace', '--tests'], { env: testEnv });
  },
  e2e: () => {
    run(npm, ['run', 'e2e', '-w', 'apps/web']);
  },
  'e2e-real': () => {
    tasks['web-build']();
    run('cargo', ['build', '-p', 'doto-server', '--bin', 'doto-server'], { env: testEnv });
    run(npm, ['run', 'e2e:real', '-w', 'apps/web'], { env: testEnv });
  },
  build: () => {
    tasks['web-build']();
    run('cargo', ['build', '--release', '-p', 'doto-server']);
  },
  'security-check': () => {
    run('node', ['scripts/secrets-scan.mjs']);
    run('node', ['scripts/license-check.mjs']);
  },
  'release-check': () => {
    tasks['format-check']();
    tasks.lint();
    tasks.typecheck();
    tasks['unit-test']();
    tasks['integration-test']();
    tasks['security-check']();
    console.log('\n✔ release-check đạt');
  },
  'desktop-dev': () => {
    tasks['web-build']();
    run('npx', ['tauri', 'dev'], { cwd: desktopDir });
  },
  'desktop-build': () => {
    tasks['web-build']();
    const extra = process.argv.slice(3);
    run('npx', ['tauri', 'build', ...extra], { cwd: desktopDir });
  },
};

const name = process.argv[2];
if (!tasks[name]) {
  console.error(`Task không hợp lệ: ${name}\nCó: ${Object.keys(tasks).join(', ')}`);
  process.exit(2);
}
tasks[name]();

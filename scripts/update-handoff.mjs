#!/usr/bin/env node
// Đồng bộ khối trạng thái tự động trong 3 file bàn giao + PROJECT_MEMORY.
// Dùng: npm run handoff -- --agent claude --summary "Đã làm X" [--next "Việc tiếp theo"]
// Script KHÔNG tự đánh dấu task hoàn thành — chỉ ghi dữ kiện kiểm chứng được (SHA, branch, file đổi, lần chạy test gần nhất).
import { execSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, appendFileSync } from 'node:fs';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, arr) => (a.startsWith('--') ? [...acc, [a.slice(2), arr[i + 1]?.startsWith('--') ? 'true' : arr[i + 1] ?? 'true']] : acc), []),
);
const agent = (args.agent ?? 'unknown').toLowerCase();
if (!['claude', 'codex', 'antigravity', 'unknown'].includes(agent)) {
  console.error('--agent phải là claude | codex | antigravity');
  process.exit(2);
}
const sh = (c) => {
  try {
    return execSync(c, { encoding: 'utf8' }).trim();
  } catch {
    return '';
  }
};
const sha = sh('git rev-parse HEAD') || '(chưa có commit)';
const branch = sh('git rev-parse --abbrev-ref HEAD') || '(không rõ)';
const dirty = sh('git status --porcelain').split('\n').filter(Boolean);
const commits = sh('git log -5 --pretty=format:"- %h %s (%cs)"');
const now = new Date().toISOString();
const testFile = 'docs/shared/LAST_TEST_RUN.md';
const lastTests = existsSync(testFile) ? readFileSync(testFile, 'utf8').trim() : '_Chưa có kết quả test được ghi (chạy `npm run release-check` rồi cập nhật docs/shared/LAST_TEST_RUN.md)._';

const block = `<!-- AUTO:START (scripts/update-handoff.mjs — không sửa tay khối này) -->
**Cập nhật tự động:** ${now} bởi \`${agent}\`
- Branch: \`${branch}\` · HEAD: \`${sha}\`
- Working tree: ${dirty.length ? `${dirty.length} file chưa commit` : 'sạch'}
${args.summary ? `- Tóm tắt phiên: ${args.summary}\n` : ''}${args.next ? `- NEXT_ACTION: ${args.next}\n` : ''}
**Commit gần nhất:**
${commits || '- (chưa có)'}

**Lần chạy test gần nhất:**
${lastTests}
<!-- AUTO:END -->`;

const targets = ['docs/agents/HANDOFF_CLAUDE.md', 'docs/agents/HANDOFF_CODEX.md', 'docs/agents/HANDOFF_ANTIGRAVITY.md', 'docs/shared/PROJECT_MEMORY.md'];
for (const f of targets) {
  if (!existsSync(f)) {
    console.error(`thiếu ${f}`);
    process.exit(1);
  }
  const s = readFileSync(f, 'utf8');
  const re = /<!-- AUTO:START[\s\S]*?<!-- AUTO:END -->/;
  writeFileSync(f, re.test(s) ? s.replace(re, block) : `${s.trimEnd()}\n\n${block}\n`);
  console.log(`✔ cập nhật ${f}`);
}
if (args.summary) {
  appendFileSync('docs/shared/CHANGELOG_DEV.md', `\n- ${now.slice(0, 10)} · ${agent} · \`${sha.slice(0, 10)}\` · ${args.summary}\n`);
  console.log('✔ ghi nối docs/shared/CHANGELOG_DEV.md');
}

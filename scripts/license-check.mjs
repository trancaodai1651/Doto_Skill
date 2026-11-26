#!/usr/bin/env node
// Kiểm tra giấy phép phụ thuộc trước khi đóng gói:
// - Rust: `cargo metadata` (toàn bộ crate được biên dịch vào binary).
// - Web: dependencies runtime của apps/web và packages/ui (được bundle vào dist).
// Từ chối GPL/AGPL/LGPL/SSPL (copyleft không tương thích với phân phối đóng gói hiện tại).
// FFmpeg KHÔNG được đóng gói — người dùng tự cài từ nguồn chính thức (xem docs/INTEGRATIONS.md).
import { execSync } from 'node:child_process';
import { readFileSync, existsSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';

const deny = /\b(A?GPL|LGPL|SSPL)(-\d|\b)/i;
const problems = [];
const rows = [];

const meta = JSON.parse(execSync('cargo metadata --format-version 1 --locked', { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 }));
const workspace = new Set(meta.workspace_members);
for (const p of meta.packages) {
  if (workspace.has(p.id)) continue;
  const lic = p.license ?? (p.license_file ? `file:${p.license_file}` : 'UNKNOWN');
  rows.push(`rust\t${p.name}\t${p.version}\t${lic}`);
  // Dual-license "MIT OR GPL" chấp nhận được vì có lựa chọn không copyleft
  const options = lic.split(/\s+OR\s+|\//);
  if (options.every((o) => deny.test(o))) problems.push(`rust ${p.name}@${p.version}: ${lic}`);
  if (lic === 'UNKNOWN') problems.push(`rust ${p.name}@${p.version}: không khai báo giấy phép`);
}

function walkNpm(pkgDir, seen) {
  const pj = JSON.parse(readFileSync(path.join(pkgDir, 'package.json'), 'utf8'));
  for (const dep of Object.keys(pj.dependencies ?? {})) {
    if (seen.has(dep)) continue;
    seen.add(dep);
    const candidates = [path.join(pkgDir, 'node_modules', dep), path.join('node_modules', dep)];
    const dir = candidates.find((c) => existsSync(path.join(c, 'package.json')));
    if (!dir) continue;
    const dpj = JSON.parse(readFileSync(path.join(dir, 'package.json'), 'utf8'));
    const lic = typeof dpj.license === 'string' ? dpj.license : dpj.license?.type ?? 'UNKNOWN';
    rows.push(`npm\t${dep}\t${dpj.version}\t${lic}`);
    if (deny.test(lic) && !/\bOR\b/.test(lic)) problems.push(`npm ${dep}@${dpj.version}: ${lic}`);
    walkNpm(dir, seen);
  }
}
for (const ws of ['apps/web', 'packages/ui']) if (existsSync(path.join(ws, 'package.json'))) walkNpm(ws, new Set());

mkdirSync('target', { recursive: true });
writeFileSync('target/THIRD_PARTY_LICENSES.tsv', 'ecosystem\tname\tversion\tlicense\n' + rows.sort().join('\n') + '\n');

if (problems.length) {
  console.error('✖ Giấy phép cần xem xét:\n' + problems.map((p) => '  - ' + p).join('\n'));
  process.exit(1);
}
console.log(`✔ license-check: ${rows.length} phụ thuộc, không có copyleft bắt buộc (danh sách: target/THIRD_PARTY_LICENSES.tsv)`);

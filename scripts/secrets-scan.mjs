#!/usr/bin/env node
// Quét bí mật trong các file được Git theo dõi (hoặc sắp commit). Thoát mã 1 nếu phát hiện.
// Mẫu: token GitHub thật (36+ ký tự), AWS key, private key PEM, Cloudflare tunnel token (base64 JSON "eyJhIjoi"), Slack, OpenAI/Anthropic key.
import { execSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';

const patterns = [
  ['GitHub token', /\b(gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{60,})\b/],
  ['AWS access key', /\bAKIA[0-9A-Z]{16}\b/],
  ['Private key', /-----BEGIN (RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/],
  ['Cloudflare tunnel token', /\beyJhIjoi[A-Za-z0-9+/=_-]{60,}/],
  ['Slack token', /\bxox[abpr]-[A-Za-z0-9-]{20,}/],
  ['OpenAI key', /\bsk-(proj-)?[A-Za-z0-9_-]{40,}\b/],
  ['Anthropic key', /\bsk-ant-[A-Za-z0-9_-]{40,}\b/],
];
const forbiddenFiles = [/(^|\/)\.env(\..*)?$/, /\.pem$/, /\.p12$/, /\.pfx$/, /(^|\/)cert\.json$/, /\.sqlite$/, /(^|\/)credentials\.json$/];
const allowEnvExample = /\.env\.example$/;

let files;
try {
  files = execSync('git ls-files -co --exclude-standard', { encoding: 'utf8' }).split('\n').filter(Boolean);
} catch {
  console.error('Cần chạy trong repo git');
  process.exit(2);
}

const findings = [];
for (const f of files) {
  if (forbiddenFiles.some((r) => r.test(f)) && !allowEnvExample.test(f)) findings.push(`${f}: loại file không được commit`);
  let st;
  try {
    st = statSync(f);
  } catch {
    continue;
  }
  if (!st.isFile() || st.size > 2_000_000) continue;
  let text;
  try {
    text = readFileSync(f, 'utf8');
  } catch {
    continue;
  }
  if (text.includes('\u0000')) continue; // nhị phân
  text.split('\n').forEach((line, i) => {
    for (const [name, re] of patterns) if (re.test(line)) findings.push(`${f}:${i + 1}: nghi ${name}`);
  });
}

if (findings.length) {
  console.error('✖ Phát hiện bí mật tiềm năng:\n' + findings.map((x) => '  - ' + x).join('\n'));
  process.exit(1);
}
console.log(`✔ secrets-scan: ${files.length} file, không phát hiện bí mật`);

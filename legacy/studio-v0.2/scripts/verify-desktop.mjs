#!/usr/bin/env node
// Kiểm tra toàn vẹn gói .desktop/ trước khi chạy tauri build trên Windows và macOS Universal.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targetDir = process.argv[2] ? path.resolve(process.argv[2]) : path.join(ROOT, ".desktop");

const mustExist = [
  path.join(targetDir, "app", "bin", "cli.mjs"),
  path.join(targetDir, "app", "server", "index.mjs"),
  path.join(targetDir, "app", "dist", "index.html"),
  path.join(targetDir, "node", process.platform === "win32" ? "node.exe" : "node"),
];

for (const f of mustExist) {
  if (!fs.existsSync(f)) {
    console.error(`✗ Thiếu tệp bắt buộc trong gói Desktop: ${f}`);
    process.exit(1);
  }
}
console.log("✓ Kiểm tra gói Desktop (.desktop) hợp lệ 100%.");

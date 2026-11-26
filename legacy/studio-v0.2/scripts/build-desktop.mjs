#!/usr/bin/env node
// Chuẩn bị tài nguyên đóng gói Tauri Desktop cho cả Windows x64 và macOS Universal (arm64 + x86_64):
//  - .desktop/app  : server ESM + bin/cli.mjs + giao diện Web đã build (dist/)
//  - .desktop/node : runtime Node.js thực thi máy chủ ngầm (trên macOS --universal sẽ ghép lipo x64 + arm64)
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, ".desktop");
const APP_OUT = path.join(OUT, "app");
const NODE_OUT = path.join(OUT, "node");
const isUniversal = process.argv.includes("--universal");

if (!fs.existsSync(path.join(ROOT, "src-tauri", "icons", "icon.ico"))) {
  execFileSync(process.execPath, [path.join(ROOT, "scripts", "generate-icons.mjs")], { cwd: ROOT, stdio: "inherit" });
}

if (!fs.existsSync(path.join(ROOT, "dist", "index.html")) || process.argv.includes("--rebuild")) {
  console.log("▸ Build giao diện Web (Vite)...");
  execFileSync(process.execPath, [path.join(ROOT, "node_modules", "vite", "bin", "vite.js"), "build"], {
    cwd: ROOT,
    stdio: "inherit",
  });
}

console.log("▸ Đóng gói mã nguồn máy chủ & giao diện → .desktop/app");
fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(APP_OUT, { recursive: true });

for (const item of ["bin", "server", "dist", "package.json"]) {
  const src = path.join(ROOT, item);
  if (fs.existsSync(src)) {
    fs.cpSync(src, path.join(APP_OUT, item), { recursive: true });
  }
}

console.log("▸ Sao chép Node runtime → .desktop/node");
fs.mkdirSync(NODE_OUT, { recursive: true });
fs.copyFileSync(process.execPath, path.join(NODE_OUT, path.basename(process.execPath)));

if (isUniversal) {
  if (process.platform !== "darwin") {
    throw new Error("Cờ --universal chỉ chạy trên máy macOS (cần lệnh lipo và codesign của Xcode Command Line Tools).");
  }
  await import("./desktop-universal.mjs");
}

console.log("✓ Tài nguyên Desktop (.desktop/) đã sẵn sàng để đóng gói Tauri!");

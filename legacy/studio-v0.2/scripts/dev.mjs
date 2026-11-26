#!/usr/bin/env node
// Khởi chạy đồng thời API Server (cổng 4320) và Vite HMR Web Server (cổng 5179) khi phát triển.
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const viteBin = path.join(ROOT, "node_modules", "vite", "bin", "vite.js");

const api = spawn(process.execPath, [path.join(ROOT, "server", "index.mjs"), "--dev"], {
  cwd: ROOT,
  stdio: "inherit",
  env: { ...process.env, DOTO_SKILL_PORT: process.env.DOTO_SKILL_PORT || "4320" },
});

const web = spawn(process.execPath, [viteBin, "--port", "5179"], {
  cwd: ROOT,
  stdio: "inherit",
  env: { ...process.env, DOTO_SKILL_PORT: process.env.DOTO_SKILL_PORT || "4320" },
});

const cleanup = () => {
  try { api.kill(); } catch {}
  try { web.kill(); } catch {}
  process.exit(0);
};

process.on("SIGINT", cleanup);
process.on("SIGTERM", cleanup);
api.on("exit", (code) => { if (code) cleanup(); });
web.on("exit", (code) => { if (code) cleanup(); });

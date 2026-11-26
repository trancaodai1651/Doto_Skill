#!/usr/bin/env node
// Điểm khởi chạy thống nhất cho cả chế độ Web (PC / iPad / iPhone / LAN / Cloudflare Tunnel) và Tauri Desktop.
import { startServer } from "../server/index.mjs";

const port = Number(process.env.DOTO_SKILL_PORT || 4320);
const host = process.argv.includes("--lan") || process.argv.includes("--tunnel")
  ? "0.0.0.0"
  : (process.env.DOTO_SKILL_HOST || "127.0.0.1");

startServer({ port, host })
  .then(({ url, tunnelManager }) => {
    console.log(`✓ Doto Skill Studio đang chạy tại ${url}`);
    if (process.argv.includes("--tunnel")) {
      const res = tunnelManager.ensureQuickWebTunnel();
      if (res.ok) {
        console.log("▸ Đã kích hoạt Cloudflare Tunnel cho trang Web này.");
      }
    }
  })
  .catch((err) => {
    console.error("✗ Không thể khởi động Doto Skill Studio:", err);
    process.exit(1);
  });

// Trình quản lý CRUD toàn diện cho Cloudflare Tunnel trên thiết bị:
//  - Hỗ trợ Quick Tunnel miễn phí (*.trycloudflare.com), Token Tunnel (Zero Trust --token) và Named Tunnel.
//  - Quét trực tiếp các tiến trình `cloudflared` đang chạy trên thiết bị (kể cả mở bên ngoài ứng dụng) để quản lý/nhận nuôi/dừng.
import { spawn, execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolveCloudflaredBinary, execCliQuiet, standaloneEnvironment } from "./cli-resolver.mjs";
import {
  loadTunnelsConfig,
  saveTunnelsConfig,
  loadTunnelsRuntime,
  saveTunnelsRuntime,
} from "./data.mjs";

const execFileAsync = promisify(execFile);
const IS_WIN = process.platform === "win32";
const TRYCF_URL_RE = /https:\/\/[a-z0-9-]+\.trycloudflare\.com/i;

async function scanDeviceCloudflaredProcesses() {
  try {
    if (IS_WIN) {
      const ps = `Get-CimInstance Win32_Process -Filter "Name='cloudflared.exe'" | Select-Object ProcessId, CommandLine, CreationDate | ConvertTo-Json -Compress`;
      const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", ps], {
        timeout: 6000,
        windowsHide: true,
      });
      const txt = String(stdout || "").trim();
      if (!txt) return [];
      const parsed = JSON.parse(txt);
      const arr = Array.isArray(parsed) ? parsed : [parsed];
      return arr
        .filter((p) => p && p.ProcessId)
        .map((p) => ({
          pid: Number(p.ProcessId),
          commandLine: String(p.CommandLine || "cloudflared.exe"),
        }));
    } else {
      const { stdout } = await execFileAsync("ps", ["-ax", "-o", "pid=,command="], { timeout: 5000 });
      return String(stdout || "")
        .split(/\r?\n/)
        .map((line) => {
          const m = line.trim().match(/^(\d+)\s+(.+)$/);
          return m ? { pid: Number(m[1]), commandLine: m[2] } : null;
        })
        .filter((x) => x && /\bcloudflared\b/.test(x.commandLine) && !x.commandLine.includes("ps -ax"));
    }
  } catch {
    return [];
  }
}

function parseTargetFromCmd(cmdLine) {
  const urlMatch = String(cmdLine || "").match(/--url\s+(?:"([^"]+)"|'([^']+)'|(\S+))/i);
  if (urlMatch) return urlMatch[1] || urlMatch[2] || urlMatch[3] || "";
  if (/--token\s+/i.test(cmdLine)) return "Zero Trust Token Tunnel";
  const runMatch = String(cmdLine || "").match(/tunnel\s+run\s+(\S+)/i);
  if (runMatch) return `Named Tunnel: ${runMatch[1]}`;
  return "";
}

function killPidCrossPlatform(pid) {
  if (!pid || Number.isNaN(Number(pid))) return false;
  try {
    if (IS_WIN) {
      spawn("taskkill", ["/PID", String(pid), "/T", "/F"], { windowsHide: true });
    } else {
      process.kill(Number(pid), "SIGTERM");
    }
    return true;
  } catch {
    try {
      process.kill(Number(pid), "SIGKILL");
      return true;
    } catch {
      return false;
    }
  }
}

export function createTunnelManager({ onEvent, defaultPort = 4320 } = {}) {
  const configs = new Map();
  const runtimes = new Map(); // id -> { proc, pid, status, publicUrl, startedAt, lastError, logs }
  const persistedRuntime = loadTunnelsRuntime();

  for (const item of loadTunnelsConfig()) {
    if (item && item.id) {
      configs.set(item.id, item);
      const prevRt = persistedRuntime[item.id];
      runtimes.set(item.id, {
        proc: null,
        pid: prevRt?.pid || null,
        status: prevRt?.pid ? "running" : "stopped",
        publicUrl: prevRt?.publicUrl || item.publicHostname || "",
        startedAt: prevRt?.startedAt || null,
        lastError: "",
        logs: [],
      });
    }
  }

  function persistConfigs() {
    saveTunnelsConfig([...configs.values()]);
  }

  function persistRuntimes() {
    const out = {};
    for (const [id, rt] of runtimes.entries()) {
      if (rt.pid && (rt.status === "running" || rt.status === "starting")) {
        out[id] = {
          pid: rt.pid,
          publicUrl: rt.publicUrl || "",
          startedAt: rt.startedAt || Date.now(),
        };
      }
    }
    saveTunnelsRuntime(out);
  }

  function serializeManaged(cfg) {
    const rt = runtimes.get(cfg.id) || {
      status: "stopped",
      pid: null,
      publicUrl: cfg.publicHostname || "",
      startedAt: null,
      lastError: "",
      logs: [],
    };
    return {
      id: cfg.id,
      name: cfg.name,
      mode: cfg.mode || "quick",
      targetUrl: cfg.targetUrl || `http://127.0.0.1:${defaultPort}`,
      hasToken: Boolean(cfg.token && cfg.token.trim()),
      tokenPreview: cfg.token ? `${cfg.token.slice(0, 10)}...` : "",
      tunnelNameOrId: cfg.tunnelNameOrId || "",
      publicHostname: cfg.publicHostname || "",
      protocol: cfg.protocol || "auto",
      autoStart: Boolean(cfg.autoStart),
      createdAt: cfg.createdAt,
      updatedAt: cfg.updatedAt,
      status: rt.status,
      pid: rt.pid || null,
      publicUrl: rt.publicUrl || cfg.publicHostname || "",
      startedAt: rt.startedAt || null,
      lastError: rt.lastError || "",
      logs: (rt.logs || []).slice(-60),
    };
  }

  function emitUpdate(type, cfg) {
    onEvent?.({
      type,
      tunnel: cfg ? serializeManaged(cfg) : null,
    });
  }

  function pushTunnelLog(id, line) {
    const rt = runtimes.get(id);
    const cfg = configs.get(id);
    if (!rt || !cfg) return;
    const clean = String(line || "").trim();
    if (!clean) return;
    rt.logs.push({ at: Date.now(), text: clean.slice(0, 500) });
    if (rt.logs.length > 80) rt.logs.splice(0, rt.logs.length - 80);

    const urlMatch = clean.match(TRYCF_URL_RE);
    if (urlMatch && rt.publicUrl !== urlMatch[0]) {
      rt.publicUrl = urlMatch[0];
      rt.status = "running";
      rt.lastError = "";
      persistRuntimes();
      emitUpdate("tunnel:updated", cfg);
    } else if (/Registered tunnel connection|Connection .* registered/i.test(clean) && rt.status === "starting") {
      rt.status = "running";
      if (!rt.publicUrl && cfg.publicHostname) rt.publicUrl = cfg.publicHostname;
      persistRuntimes();
      emitUpdate("tunnel:updated", cfg);
    }
  }

  async function reconcileDeviceProcesses() {
    const osProcs = await scanDeviceCloudflaredProcesses();
    const alivePids = new Set(osProcs.map((p) => p.pid));

    for (const [id, rt] of runtimes.entries()) {
      if (!rt.proc && rt.pid && !alivePids.has(rt.pid)) {
        rt.pid = null;
        rt.status = "stopped";
        persistRuntimes();
      }
    }

    const managedPids = new Set(
      [...runtimes.values()].map((r) => r.pid).filter(Boolean)
    );

    const external = osProcs
      .filter((p) => !managedPids.has(p.pid))
      .map((p) => ({
        pid: p.pid,
        commandLine: p.commandLine,
        targetHint: parseTargetFromCmd(p.commandLine),
      }));

    return external;
  }

  async function getOverview() {
    const bin = resolveCloudflaredBinary();
    const [verRes, external] = await Promise.all([
      bin ? execCliQuiet(bin, ["--version"], 5000) : Promise.resolve({ ok: false, stdout: "" }),
      reconcileDeviceProcesses(),
    ]);
    return {
      installed: Boolean(bin),
      binaryPath: bin?.rawPath || "",
      version: verRes.stdout.split(/\r?\n/)[0] || "",
      defaultWebUrl: `http://127.0.0.1:${defaultPort}`,
      managed: [...configs.values()]
        .sort((a, b) => b.createdAt - a.createdAt)
        .map(serializeManaged),
      external,
    };
  }

  function startTunnel(id) {
    const cfg = configs.get(id);
    if (!cfg) return { ok: false, error: "Không tìm thấy cấu hình Cloudflare Tunnel." };
    const bin = resolveCloudflaredBinary();
    if (!bin) {
      return {
        ok: false,
        error: "Chưa cài `cloudflared` trên thiết bị. Hãy cài bằng `winget install Cloudflare.cloudflared` hoặc `brew install cloudflared`.",
      };
    }

    let rt = runtimes.get(id);
    if (rt && (rt.status === "running" || rt.status === "starting") && rt.pid) {
      return { ok: true, tunnel: serializeManaged(cfg) };
    }

    const args = ["tunnel", "--no-autoupdate"];
    if (cfg.protocol && cfg.protocol !== "auto") {
      args.push("--protocol", cfg.protocol);
    }

    if (cfg.mode === "token") {
      if (!cfg.token || !cfg.token.trim()) {
        return { ok: false, error: "Chế độ Token yêu cầu nhập Cloudflare Tunnel Token (--token)." };
      }
      args.push("run", "--token", cfg.token.trim());
    } else if (cfg.mode === "named") {
      if (!cfg.tunnelNameOrId || !cfg.tunnelNameOrId.trim()) {
        return { ok: false, error: "Chế độ Named Tunnel yêu cầu nhập Tên hoặc UUID của Tunnel." };
      }
      args.push("run", cfg.tunnelNameOrId.trim());
    } else {
      // Quick Tunnel (*.trycloudflare.com)
      args.push("--url", cfg.targetUrl || `http://127.0.0.1:${defaultPort}`);
    }

    rt = {
      proc: null,
      pid: null,
      status: "starting",
      publicUrl: cfg.mode === "quick" ? "" : cfg.publicHostname || "",
      startedAt: Date.now(),
      lastError: "",
      logs: [{ at: Date.now(), text: `Khởi chạy: cloudflared ${args.map((a) => (a === cfg.token ? "***" : a)).join(" ")}` }],
    };
    runtimes.set(id, rt);

    let proc;
    try {
      proc = spawn(bin.file, [...bin.prefixArgs, ...args], {
        env: standaloneEnvironment(),
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (err) {
      rt.status = "error";
      rt.lastError = err?.message || String(err);
      emitUpdate("tunnel:updated", cfg);
      return { ok: false, error: rt.lastError };
    }

    rt.proc = proc;
    rt.pid = proc.pid;
    persistRuntimes();
    emitUpdate("tunnel:updated", cfg);

    const handleStream = (chunk) => {
      const text = chunk.toString("utf8");
      for (const line of text.split(/\r?\n/)) {
        if (line.trim()) pushTunnelLog(id, line);
      }
    };

    proc.stdout.on("data", handleStream);
    proc.stderr.on("data", handleStream);

    proc.on("error", (err) => {
      rt.status = "error";
      rt.lastError = err?.message || String(err);
      rt.proc = null;
      rt.pid = null;
      persistRuntimes();
      emitUpdate("tunnel:updated", cfg);
    });

    proc.on("close", (code) => {
      rt.proc = null;
      rt.pid = null;
      if (rt.status !== "stopped") {
        rt.status = code === 0 ? "stopped" : "error";
        if (code && code !== 0 && !rt.lastError) {
          rt.lastError = `cloudflared dừng với mã ${code}`;
        }
      }
      persistRuntimes();
      emitUpdate("tunnel:updated", cfg);
    });

    return { ok: true, tunnel: serializeManaged(cfg) };
  }

  function stopTunnel(id) {
    const cfg = configs.get(id);
    if (!cfg) return { ok: false, error: "Không tìm thấy Tunnel." };
    const rt = runtimes.get(id);
    if (!rt || (!rt.pid && !rt.proc)) {
      if (rt) rt.status = "stopped";
      persistRuntimes();
      return { ok: true, tunnel: serializeManaged(cfg) };
    }

    rt.status = "stopped";
    if (rt.pid) killPidCrossPlatform(rt.pid);
    try { rt.proc?.kill(); } catch {}
    rt.proc = null;
    rt.pid = null;
    persistRuntimes();
    emitUpdate("tunnel:updated", cfg);
    return { ok: true, tunnel: serializeManaged(cfg) };
  }

  function createTunnel(input = {}) {
    const id = `tun_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 6)}`;
    const mode = ["quick", "token", "named"].includes(input.mode) ? input.mode : "quick";
    const cfg = {
      id,
      name: String(input.name || "Doto Skill Web Tunnel").trim(),
      mode,
      targetUrl: String(input.targetUrl || `http://127.0.0.1:${defaultPort}`).trim(),
      token: String(input.token || "").trim(),
      tunnelNameOrId: String(input.tunnelNameOrId || "").trim(),
      publicHostname: String(input.publicHostname || "").trim(),
      protocol: ["auto", "http2", "quic"].includes(input.protocol) ? input.protocol : "auto",
      autoStart: Boolean(input.autoStart),
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    configs.set(id, cfg);
    runtimes.set(id, {
      proc: null,
      pid: null,
      status: "stopped",
      publicUrl: cfg.publicHostname || "",
      startedAt: null,
      lastError: "",
      logs: [],
    });
    persistConfigs();
    emitUpdate("tunnel:created", cfg);

    if (input.startNow !== false) {
      return startTunnel(id);
    }
    return { ok: true, tunnel: serializeManaged(cfg) };
  }

  function updateTunnel(id, patch = {}) {
    const cur = configs.get(id);
    if (!cur) return { ok: false, error: "Không tìm thấy Tunnel cần cập nhật." };

    const next = {
      ...cur,
      name: patch.name !== undefined ? String(patch.name).trim() || cur.name : cur.name,
      mode: patch.mode && ["quick", "token", "named"].includes(patch.mode) ? patch.mode : cur.mode,
      targetUrl: patch.targetUrl !== undefined ? String(patch.targetUrl).trim() || cur.targetUrl : cur.targetUrl,
      token: patch.token !== undefined && patch.token !== "" ? String(patch.token).trim() : cur.token,
      tunnelNameOrId: patch.tunnelNameOrId !== undefined ? String(patch.tunnelNameOrId).trim() : cur.tunnelNameOrId,
      publicHostname: patch.publicHostname !== undefined ? String(patch.publicHostname).trim() : cur.publicHostname,
      protocol: patch.protocol && ["auto", "http2", "quic"].includes(patch.protocol) ? patch.protocol : cur.protocol,
      autoStart: patch.autoStart !== undefined ? Boolean(patch.autoStart) : cur.autoStart,
      updatedAt: Date.now(),
    };
    configs.set(id, next);
    persistConfigs();

    const rt = runtimes.get(id);
    const wasRunning = rt && (rt.status === "running" || rt.status === "starting");
    const runtimeChanged =
      cur.mode !== next.mode ||
      cur.targetUrl !== next.targetUrl ||
      cur.token !== next.token ||
      cur.tunnelNameOrId !== next.tunnelNameOrId ||
      cur.protocol !== next.protocol;

    if (wasRunning && (runtimeChanged || patch.restart)) {
      stopTunnel(id);
      return startTunnel(id);
    }

    emitUpdate("tunnel:updated", next);
    return { ok: true, tunnel: serializeManaged(next) };
  }

  function deleteTunnel(id) {
    const cur = configs.get(id);
    if (!cur) return { ok: false, error: "Không tìm thấy Tunnel cần xoá." };
    stopTunnel(id);
    configs.delete(id);
    runtimes.delete(id);
    persistConfigs();
    persistRuntimes();
    onEvent?.({ type: "tunnel:deleted", id });
    return { ok: true, id };
  }

  /** Bấm 1 chạm: Mở ngay Cloudflare Tunnel cho chính trang Web Doto Skill Studio đang chạy */
  function ensureQuickWebTunnel() {
    const target = `http://127.0.0.1:${defaultPort}`;
    for (const cfg of configs.values()) {
      if (cfg.mode === "quick" && (cfg.targetUrl === target || cfg.targetUrl === `http://localhost:${defaultPort}`)) {
        return startTunnel(cfg.id);
      }
    }
    return createTunnel({
      name: `Doto Skill Studio Web (Port ${defaultPort})`,
      mode: "quick",
      targetUrl: target,
      protocol: "http2",
      autoStart: true,
      startNow: true,
    });
  }

  /** Nhận nuôi một tiến trình cloudflared đang chạy bên ngoài vào danh sách CRUD quản lý */
  async function adoptExternalProcess(pid, customName = "") {
    const external = await reconcileDeviceProcesses();
    const found = external.find((x) => x.pid === Number(pid));
    if (!found) return { ok: false, error: `Không tìm thấy tiến trình cloudflared PID ${pid}.` };

    const targetHint = found.targetHint;
    const isToken = targetHint === "Zero Trust Token Tunnel";
    const id = `tun_${Date.now().toString(36)}_ext`;
    const cfg = {
      id,
      name: customName.trim() || `Adopted Tunnel (PID ${found.pid})`,
      mode: isToken ? "token" : "quick",
      targetUrl: targetHint.startsWith("http") ? targetHint : `http://127.0.0.1:${defaultPort}`,
      token: "",
      tunnelNameOrId: "",
      publicHostname: "",
      protocol: "auto",
      autoStart: false,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    configs.set(id, cfg);
    runtimes.set(id, {
      proc: null,
      pid: found.pid,
      status: "running",
      publicUrl: "",
      startedAt: Date.now(),
      lastError: "",
      logs: [{ at: Date.now(), text: `Đã gắn kết tiến trình có sẵn PID ${found.pid}: ${found.commandLine}` }],
    });
    persistConfigs();
    persistRuntimes();
    emitUpdate("tunnel:created", cfg);
    return { ok: true, tunnel: serializeManaged(cfg) };
  }

  function stopExternalProcess(pid) {
    const ok = killPidCrossPlatform(Number(pid));
    return { ok };
  }

  // Tự động bật lại các Tunnel được đánh dấu autoStart nếu chưa chạy
  for (const cfg of configs.values()) {
    const rt = runtimes.get(cfg.id);
    if (cfg.autoStart && (!rt || !rt.pid)) {
      startTunnel(cfg.id);
    }
  }

  return {
    getOverview,
    createTunnel,
    updateTunnel,
    deleteTunnel,
    startTunnel,
    stopTunnel,
    ensureQuickWebTunnel,
    adoptExternalProcess,
    stopExternalProcess,
  };
}

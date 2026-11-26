// Bộ phân giải thực thi CLI đa nền tảng (Windows x64/ARM64 + macOS Universal Intel/Apple Silicon + Linux).
// Hỗ trợ Claude Code, Codex, Antigravity, Cloudflared và công cụ giải nén UnRAR / 7z / bsdtar.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const HOME = os.homedir();
const IS_WIN = process.platform === "win32";
const IS_MAC = process.platform === "darwin";

export function augmentedPath(baseEnv = process.env) {
  const sep = path.delimiter;
  const current = String(baseEnv.PATH || baseEnv.Path || "").split(sep).filter(Boolean);
  const extra = IS_WIN
    ? [
        path.join(process.env.APPDATA || path.join(HOME, "AppData", "Roaming"), "npm"),
        path.join(process.env.LOCALAPPDATA || path.join(HOME, "AppData", "Local"), "Programs", "antigravity", "bin"),
        path.join(process.env.LOCALAPPDATA || path.join(HOME, "AppData", "Local"), "Programs", "antigravity"),
        path.join(process.env.APPDATA || path.join(HOME, "AppData", "Roaming"), "Antigravity", "bin"),
        path.join(process.env.LOCALAPPDATA || path.join(HOME, "AppData", "Local"), "Programs", "claude"),
        "C:\\Program Files (x86)\\cloudflared",
        "C:\\Program Files\\cloudflared",
        "C:\\Program Files\\WinRAR",
        "C:\\Program Files (x86)\\WinRAR",
        "C:\\Program Files\\7-Zip",
        path.join(HOME, ".local", "bin"),
        path.join(HOME, ".cargo", "bin"),
      ]
    : [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        path.join(HOME, ".local", "bin"),
        path.join(HOME, ".npm-global", "bin"),
        path.join(HOME, ".cargo", "bin"),
        ...(IS_MAC
          ? [
              "/Applications/Antigravity.app/Contents/Resources/app/bin",
              "/Applications/Antigravity.app/Contents/Resources/bin",
              "/Applications/Codex.app/Contents/Resources",
            ]
          : []),
      ];

  if (!IS_WIN) {
    const nvmDir = path.join(HOME, ".nvm", "versions", "node");
    try {
      if (fs.existsSync(nvmDir)) {
        const vers = fs.readdirSync(nvmDir).sort().reverse();
        for (const v of vers.slice(0, 2)) extra.push(path.join(nvmDir, v, "bin"));
      }
    } catch {
      // ignore
    }
  }

  const seen = new Set();
  const merged = [];
  for (const dir of [...current, ...extra]) {
    const norm = IS_WIN ? dir.toLowerCase() : dir;
    if (!seen.has(norm) && fs.existsSync(dir)) {
      seen.add(norm);
      merged.push(dir);
    }
  }
  return merged.join(sep);
}

/** Môi trường sạch không bị kẹt nested-session guard của Claude Code / Codex / Antigravity */
export function standaloneEnvironment(extra = {}) {
  const next = { ...process.env, ...extra };
  for (const k of [
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_PARENT_SESSION_ID",
  ]) {
    delete next[k];
  }
  const fullPath = augmentedPath(next);
  next.PATH = fullPath;
  if (IS_WIN) next.Path = fullPath;
  return next;
}

/** Bóc tách shim .cmd của npm trên Windows thành file .exe hoặc .js thực sự */
export function unwrapWindowsCmd(cmdFile) {
  try {
    const content = fs.readFileSync(cmdFile, "utf8");
    const dir = path.dirname(cmdFile);
    const exeMatch = content.match(/"%dp0%[\\/]([^"]+\.exe)"/i);
    if (exeMatch) {
      const targetExe = path.resolve(dir, exeMatch[1]);
      if (fs.existsSync(targetExe)) return { file: targetExe, prefixArgs: [], rawPath: targetExe };
    }
    const jsMatch = content.match(/"%dp0%[\\/]([^"]+\.(?:m|c)?js)"/i);
    if (jsMatch) {
      const targetJs = path.resolve(dir, jsMatch[1]);
      if (fs.existsSync(targetJs)) return { file: process.execPath, prefixArgs: [targetJs], rawPath: targetJs };
    }
  } catch {
    // fallback bên dưới
  }
  return {
    file: process.env.ComSpec || "cmd.exe",
    prefixArgs: ["/d", "/s", "/c", cmdFile],
    rawPath: cmdFile,
  };
}

/** Tìm file thực thi của một CLI theo tên ('claude', 'codex', 'agy', 'cloudflared', ...) */
export function resolveCliBinary(name) {
  const searchDirs = augmentedPath().split(path.delimiter).filter(Boolean);

  if (name === "codex") {
    try {
      const cfg = fs.readFileSync(path.join(process.env.CODEX_HOME || path.join(HOME, ".codex"), "config.toml"), "utf8");
      const m = cfg.match(/CODEX_CLI_PATH\s*=\s*['"]([^'"]+)['"]/);
      if (m && fs.existsSync(m[1])) {
        return { file: m[1], prefixArgs: [], rawPath: m[1] };
      }
    } catch {
      // ignore
    }
  }

  const exts = IS_WIN ? [".exe", ".cmd", ""] : [""];
  for (const dir of searchDirs) {
    for (const ext of exts) {
      const candidate = path.join(dir, `${name}${ext}`);
      if (!fs.existsSync(candidate)) continue;
      try {
        if (!fs.statSync(candidate).isFile()) continue;
      } catch {
        continue;
      }
      if (IS_WIN && candidate.toLowerCase().endsWith(".cmd")) {
        return unwrapWindowsCmd(candidate);
      }
      if (/\.(?:m|c)?js$/i.test(candidate)) {
        return { file: process.execPath, prefixArgs: [candidate], rawPath: candidate };
      }
      return { file: candidate, prefixArgs: [], rawPath: candidate };
    }
  }
  return null;
}

export function resolveCloudflaredBinary() {
  const found = resolveCliBinary("cloudflared");
  if (found) return found;
  const fallbacks = IS_WIN
    ? [
        "C:\\Program Files (x86)\\cloudflared\\cloudflared.exe",
        "C:\\Program Files\\cloudflared\\cloudflared.exe",
        path.join(process.env.LOCALAPPDATA || "", "Microsoft", "WinGet", "Links", "cloudflared.exe"),
      ]
    : ["/opt/homebrew/bin/cloudflared", "/usr/local/bin/cloudflared", "/usr/bin/cloudflared"];
  for (const f of fallbacks) {
    if (f && fs.existsSync(f)) return { file: f, prefixArgs: [], rawPath: f };
  }
  return null;
}

export function resolveUnrarBinary() {
  for (const cmd of ["unrar", "UnRAR", "unar", "7z"]) {
    const found = resolveCliBinary(cmd);
    if (found) return { ...found, tool: cmd.toLowerCase() };
  }
  if (IS_WIN) {
    const winCandidates = [
      { path: "C:\\Program Files\\WinRAR\\UnRAR.exe", tool: "unrar" },
      { path: "C:\\Program Files (x86)\\WinRAR\\UnRAR.exe", tool: "unrar" },
      { path: "C:\\Program Files\\WinRAR\\WinRAR.exe", tool: "winrar" },
      { path: "C:\\Program Files\\7-Zip\\7z.exe", tool: "7z" },
    ];
    for (const c of winCandidates) {
      if (fs.existsSync(c.path)) return { file: c.path, prefixArgs: [], rawPath: c.path, tool: c.tool };
    }
  }
  return null;
}

/** Tìm ứng dụng Desktop Antigravity (Windows .exe hoặc macOS .app) */
export function resolveAntigravityDesktopApp() {
  if (IS_WIN) {
    const candidates = [
      path.join(process.env.LOCALAPPDATA || path.join(HOME, "AppData", "Local"), "Programs", "antigravity", "Antigravity.exe"),
      "C:\\Program Files\\Antigravity\\Antigravity.exe",
    ];
    return candidates.find((p) => fs.existsSync(p)) || null;
  }
  if (IS_MAC) {
    const candidates = [
      "/Applications/Antigravity.app",
      path.join(HOME, "Applications", "Antigravity.app"),
    ];
    return candidates.find((p) => fs.existsSync(p)) || null;
  }
  return null;
}

export async function execCliQuiet(bin, args, timeout = 8000) {
  if (!bin) return { ok: false, stdout: "", stderr: "missing binary" };
  try {
    const { stdout, stderr } = await execFileAsync(
      bin.file,
      [...bin.prefixArgs, ...args],
      {
        timeout,
        windowsHide: true,
        maxBuffer: 8 * 1024 * 1024,
        env: standaloneEnvironment(),
      }
    );
    return { ok: true, stdout: String(stdout || "").trim(), stderr: String(stderr || "").trim() };
  } catch (err) {
    return {
      ok: false,
      stdout: String(err?.stdout || "").trim(),
      stderr: String(err?.stderr || err?.message || "").trim(),
    };
  }
}

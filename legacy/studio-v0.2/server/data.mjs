// Quản lý thư mục dữ liệu, cấu hình người dùng, đường dẫn lưu tuỳ chỉnh, giao diện Sáng/Tối, ngôn ngữ VI/EN và dữ liệu Cloudflare Tunnel.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const HOME = os.homedir();

export function getDataDir() {
  const dir = process.env.DOTO_SKILL_DATA_DIR || path.join(HOME, ".doto-skill");
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function detectDefaultSkillRoot() {
  if (process.env.DOTO_SKILL_ROOT && fs.existsSync(process.env.DOTO_SKILL_ROOT)) {
    return path.resolve(process.env.DOTO_SKILL_ROOT);
  }
  const candidates = [
    "E:\\_TRAN CAO DAI\\SKILL",
    "D:\\_TRAN CAO DAI\\SKILL",
    path.join(HOME, "SKILL"),
    path.join(HOME, "Skills"),
    path.join(HOME, "Code", "SKILL"),
    path.join(HOME, "Documents", "SKILL"),
  ];
  for (const c of candidates) {
    if (fs.existsSync(path.join(c, "news-video", "SKILL.md")) || fs.existsSync(path.join(c, "AGENTS.md"))) {
      return path.resolve(c);
    }
  }
  const linkRoots = [
    path.join(HOME, ".claude", "skills", "news-video"),
    path.join(HOME, ".codex", "skills", "news-video"),
    path.join(HOME, ".gemini", "config", "skills", "news-video"),
  ];
  for (const lk of linkRoots) {
    try {
      if (fs.existsSync(lk)) {
        const real = fs.realpathSync(lk);
        const parent = path.dirname(real);
        if (fs.existsSync(path.join(parent, "news-video"))) return parent;
      }
    } catch {
      // bỏ qua nếu không đọc được symlink
    }
  }
  return process.platform === "win32" ? "E:\\_TRAN CAO DAI\\SKILL" : path.join(HOME, "SKILL");
}

const DEFAULT_SETTINGS = () => {
  const skillRoot = detectDefaultSkillRoot();
  return {
    skillRoot,
    customSkillSaveDir: skillRoot,
    extraSkillRoots: [],
    videoOutputDir: path.join(skillRoot, "_video"),
    mediaUploadDir: path.join(skillRoot, "_video", "_mobile_uploads"),
    theme: "dark",
    language: "vi",
    autoInjectSharedMemory: true,
    autoLogHandoff: true,
    syncSkillLinks: true,
    selectedSkill: "stickman-video",
    selectedProvider: "claude",
    providerPrefs: {
      claude: {
        model: "claude-opus-5-5",
        effort: "high",
        permissionMode: "bypassPermissions",
      },
      codex: {
        model: "gpt-6.2-sol",
        effort: "high",
        sandbox: "danger-full-access",
      },
      antigravity: {
        model: "gemini-4-carbon",
        effort: "high",
        permissionMode: "dangerously-skip-permissions",
        lsAddress: process.env.ANTIGRAVITY_LS_ADDRESS || "",
        csrfToken: process.env.ANTIGRAVITY_CSRF_TOKEN || "",
      },
    },
  };
};

export function loadSettings() {
  const file = path.join(getDataDir(), "settings.json");
  const base = DEFAULT_SETTINGS();
  try {
    const saved = JSON.parse(fs.readFileSync(file, "utf8"));
    const mergedSkillRoot = saved.skillRoot || base.skillRoot;
    const extraRoots = Array.isArray(saved.extraSkillRoots)
      ? [...new Set(saved.extraSkillRoots.map((p) => String(p || "").trim()).filter(Boolean))]
      : [];
    return {
      ...base,
      ...saved,
      skillRoot: mergedSkillRoot,
      customSkillSaveDir: saved.customSkillSaveDir || mergedSkillRoot,
      extraSkillRoots: extraRoots,
      videoOutputDir: saved.videoOutputDir || path.join(mergedSkillRoot, "_video"),
      mediaUploadDir: saved.mediaUploadDir || path.join(saved.videoOutputDir || path.join(mergedSkillRoot, "_video"), "_mobile_uploads"),
      theme: saved.theme === "light" ? "light" : "dark",
      language: saved.language === "en" ? "en" : "vi",
      providerPrefs: {
        claude: { ...base.providerPrefs.claude, ...(saved.providerPrefs?.claude || {}) },
        codex: { ...base.providerPrefs.codex, ...(saved.providerPrefs?.codex || {}) },
        antigravity: { ...base.providerPrefs.antigravity, ...(saved.providerPrefs?.antigravity || {}) },
      },
    };
  } catch {
    return base;
  }
}

export function saveSettings(patch) {
  const cur = loadSettings();
  const nextExtra = patch.extraSkillRoots !== undefined
    ? [...new Set((patch.extraSkillRoots || []).map((p) => String(p || "").trim()).filter(Boolean))]
    : cur.extraSkillRoots;

  if (patch.customSkillSaveDir && patch.customSkillSaveDir.trim()) {
    const resolvedSave = path.resolve(patch.customSkillSaveDir.trim());
    const resolvedRoot = path.resolve((patch.skillRoot || cur.skillRoot || "").trim() || ".");
    const normSave = process.platform === "win32" ? resolvedSave.toLowerCase() : resolvedSave;
    const normRoot = process.platform === "win32" ? resolvedRoot.toLowerCase() : resolvedRoot;
    if (normSave !== normRoot && !nextExtra.some((x) => (process.platform === "win32" ? path.resolve(x).toLowerCase() : path.resolve(x)) === normSave)) {
      nextExtra.push(resolvedSave);
    }
  }

  const next = {
    ...cur,
    ...patch,
    extraSkillRoots: nextExtra,
    providerPrefs: {
      claude: { ...cur.providerPrefs.claude, ...(patch.providerPrefs?.claude || {}) },
      codex: { ...cur.providerPrefs.codex, ...(patch.providerPrefs?.codex || {}) },
      antigravity: { ...cur.providerPrefs.antigravity, ...(patch.providerPrefs?.antigravity || {}) },
    },
  };
  const file = path.join(getDataDir(), "settings.json");
  fs.writeFileSync(file, JSON.stringify(next, null, 2), "utf8");
  return next;
}

export function getAllowedRoots(settings = loadSettings(), extraDirs = []) {
  const raw = [
    settings?.skillRoot,
    settings?.customSkillSaveDir,
    ...(settings?.extraSkillRoots || []),
    settings?.videoOutputDir,
    settings?.mediaUploadDir,
    getDataDir(),
    ...extraDirs,
  ];
  const out = [];
  const seen = new Set();
  for (const r of raw) {
    if (!r || !String(r).trim()) continue;
    const resolved = path.resolve(String(r).trim());
    const norm = process.platform === "win32" ? resolved.toLowerCase() : resolved;
    if (!seen.has(norm)) {
      seen.add(norm);
      out.push(resolved);
    }
  }
  return out;
}

export function loadJobsHistory() {
  const file = path.join(getDataDir(), "jobs-history.json");
  try {
    const arr = JSON.parse(fs.readFileSync(file, "utf8"));
    return Array.isArray(arr) ? arr.slice(0, 100) : [];
  } catch {
    return [];
  }
}

export function saveJobsHistory(jobs) {
  const file = path.join(getDataDir(), "jobs-history.json");
  try {
    fs.writeFileSync(file, JSON.stringify(jobs.slice(0, 100), null, 2), "utf8");
  } catch {
    // bỏ qua lỗi ghi phụ
  }
}

export function loadTunnelsConfig() {
  const file = path.join(getDataDir(), "tunnels.json");
  try {
    const arr = JSON.parse(fs.readFileSync(file, "utf8"));
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

export function saveTunnelsConfig(tunnels) {
  const file = path.join(getDataDir(), "tunnels.json");
  fs.writeFileSync(file, JSON.stringify(tunnels, null, 2), "utf8");
  return tunnels;
}

export function loadTunnelsRuntime() {
  const file = path.join(getDataDir(), "tunnels-runtime.json");
  try {
    const obj = JSON.parse(fs.readFileSync(file, "utf8"));
    return obj && typeof obj === "object" ? obj : {};
  } catch {
    return {};
  }
}

export function saveTunnelsRuntime(runtimeMap) {
  const file = path.join(getDataDir(), "tunnels-runtime.json");
  try {
    fs.writeFileSync(file, JSON.stringify(runtimeMap, null, 2), "utf8");
  } catch {
    // ignore
  }
}

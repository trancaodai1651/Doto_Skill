// Quản lý danh mục 3 AI (Claude Code · Codex · Antigravity), tự động nạp danh sách Model của từng AI
// và các mức Effort chính xác tương ứng với từng Model.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  resolveCliBinary,
  resolveAntigravityDesktopApp,
  execCliQuiet,
} from "./cli-resolver.mjs";

const HOME = os.homedir();

export const EFFORT_LABELS = {
  low: {
    label: "Low",
    vi: "Nhanh · Nhẹ",
    desc: "Phản hồi nhanh, suy luận nhẹ — hợp sửa nhanh câu chữ hoặc kiểm tra cú pháp.",
  },
  medium: {
    label: "Medium",
    vi: "Cân bằng",
    desc: "Cân bằng tốc độ và độ sâu suy luận — hợp làm bản tin hằng ngày.",
  },
  high: {
    label: "High",
    vi: "Sâu · Chuẩn",
    desc: "Suy luận sâu nhiều bước, kiểm chứng chéo nguồn và bố cục — khuyên dùng tạo video trọn gói.",
  },
  xhigh: {
    label: "Extra High",
    vi: "Siêu sâu (xHigh)",
    desc: "Ngân sách tư duy mở rộng — hợp kịch bản điều tra phức tạp, sửa code engine hoặc cảnh 3D.",
  },
  max: {
    label: "Max",
    vi: "Tối đa (Max)",
    desc: "Mức suy luận tối đa của mô hình cho bài toán khó nhất.",
  },
  ultra: {
    label: "Ultra",
    vi: "Cực đại (Ultra)",
    desc: "Suy luận tối đa kết hợp tự động phân tách tác vụ con (Codex GPT-6).",
  },
};

export const CLAUDE_MODELS = [
  {
    id: "claude-opus-5-5",
    name: "Claude Opus 5.5",
    badge: "Khuyên dùng trọn gói",
    defaultEffort: "high",
    efforts: ["low", "medium", "high", "xhigh", "max"],
    desc: "Mạnh nhất khi chạy trọn quy trình: thu tin → đối chiếu 2 nguồn → giọng VieNeu → render → tự đọc tờ ảnh .sheet.jpg.",
  },
  {
    id: "claude-fable-5-1",
    name: "Claude Fable 5.1",
    badge: "Kịch bản & Viral",
    defaultEffort: "high",
    efforts: ["low", "medium", "high", "xhigh", "max"],
    desc: " Tối ưu lối kể chuyện cuốn hút, viết hook 3 giây đầu và lời thoại nhân vật người que tự nhiên.",
  },
  {
    id: "claude-sonnet-5-5",
    name: "Claude Sonnet 5.5",
    badge: "Nhanh · Cân bằng",
    defaultEffort: "medium",
    efforts: ["low", "medium", "high", "xhigh", "max"],
    desc: "Tốc độ nhanh, chi phí hợp lý cho sản xuất bản tin hằng ngày 60 giây.",
  },
  {
    id: "claude-haiku-5-5",
    name: "Claude Haiku 5.5",
    badge: "Siêu tốc",
    defaultEffort: "low",
    efforts: ["low", "medium", "high", "xhigh", "max"],
    desc: "Phản hồi tức thì cho các lần chỉnh nhỏ từng cảnh hoặc đọc lại 1 câu thoại.",
  },
];

export const FALLBACK_CODEX_MODELS = [
  {
    id: "gpt-6.2-sol",
    name: "GPT-6.2-Sol",
    badge: "Mặc định mới nhất",
    defaultEffort: "high",
    efforts: ["low", "medium", "high", "xhigh", "max", "ultra"],
    desc: "Mô hình chủ lực thế hệ GPT-6.2 cho lập trình Remotion/HyperFrames và tự động hoá chuỗi script.",
  },
  {
    id: "gpt-6.2-astra",
    name: "GPT-6.2-Astra",
    badge: "Suy luận sâu",
    defaultEffort: "high",
    efforts: ["low", "medium", "high", "xhigh", "max", "ultra"],
    desc: "Chuyên xử lý kiến trúc phức tạp, dựng rig 3D và kiểm định logic nhiều bước.",
  },
  {
    id: "gpt-6.1-terra",
    name: "GPT-6.1 Terra",
    badge: "Đa năng",
    defaultEffort: "medium",
    efforts: ["low", "medium", "high", "xhigh", "max", "ultra"],
    desc: "Ổn định cho chạy hàng loạt nhiều bản tin theo mẫu.",
  },
  {
    id: "gpt-6.1-sol",
    name: "GPT-6.1-Sol",
    badge: "Nhanh",
    defaultEffort: "medium",
    efforts: ["low", "medium", "high", "xhigh", "max", "ultra"],
    desc: "Tốc độ cao cho biên tập storyboard và chạy lệnh kiểm tra.",
  },
  {
    id: "gpt-6-luna",
    name: "GPT-6-Luna",
    badge: "Gọn nhẹ",
    defaultEffort: "medium",
    efforts: ["low", "medium", "high", "xhigh", "max"],
    desc: "Bản nhẹ tối ưu tốc độ xử lý nhanh (hỗ trợ từ low tới max).",
  },
];

export const DEFAULT_ANTIGRAVITY_MODELS = [
  {
    id: "gemini-4-carbon",
    name: "Gemini 4 Carbon",
    badge: "Chuẩn Pro · Đa phương thức",
    defaultEffort: "high",
    efforts: ["low", "medium", "high"],
    slugPattern: "gemini-4-carbon-{effort}",
    desc: "Mô hình chủ lực trên Google AI Pro: nhìn tờ ảnh trực quan cực chuẩn, điều phối công cụ mượt mà.",
  },
  {
    id: "gemini-4-neon",
    name: "Gemini 4 Neon",
    badge: "Suy luận sâu nhất",
    defaultEffort: "high",
    efforts: ["low", "medium", "high", "xhigh"],
    slugPattern: "gemini-4-neon-{effort}",
    desc: "Đỉnh cao suy luận của Google DeepMind, hỗ trợ thêm mức Extra High (xhigh) cho dự án lớn.",
  },
  {
    id: "gemini-4-hydrogen",
    name: "Gemini 4 Hydrogen",
    badge: "Nhanh · Mọi gói",
    defaultEffort: "medium",
    efforts: ["low", "medium", "high", "xhigh"],
    slugPattern: "gemini-4-hydrogen-{effort}",
    desc: "Nhanh, khả dụng trên mọi gói tài khoản, hỗ trợ đầy đủ từ Low tới Extra High (xhigh).",
  },
  {
    id: "gemini-pro-orchestrator-neon",
    name: "Orchestrator (Neon)",
    badge: "Điều phối kép",
    defaultEffort: "",
    efforts: [],
    fixedEffortLabel: "Tự động điều phối (Planner + Executor)",
    desc: "Ghép cặp mô hình lập kế hoạch đỉnh cao với mô hình thực thi tốc độ cao trong cùng phiên.",
  },
  {
    id: "gemini-pro-orchestrator",
    name: "Orchestrator (Carbon)",
    badge: "Điều phối chuẩn",
    defaultEffort: "",
    efforts: [],
    fixedEffortLabel: "Tự động điều phối (Planner + Executor)",
    desc: "Tự động phân bổ bước lập kế hoạch (Carbon High) và bước thực thi (Hydrogen High).",
  },
  {
    id: "gemini-3.8-flash",
    name: "Gemini 3.8 Flash",
    badge: "Tốc độ cao",
    defaultEffort: "high",
    efforts: ["medium", "high"],
    slugPattern: "gemini-3.8-flash-{effort}",
    desc: "Tốc độ phản hồi cực nhanh cho bản nháp và kiểm tra bố cục.",
  },
  {
    id: "gemini-3.1-pro",
    name: "Gemini 3.1 Pro",
    badge: "Ổn định",
    defaultEffort: "high",
    efforts: ["high"],
    slugPattern: "gemini-3.1-pro-{effort}",
    desc: "Phiên bản ổn định cho quy trình doanh nghiệp.",
  },
  {
    id: "claude-opus-5-5-thinking",
    name: "Claude Opus 5.5 (Thinking)",
    badge: "Đối tác trong AGY",
    defaultEffort: "high",
    efforts: ["high"],
    desc: "Chạy Claude Opus 5.5 có tư duy ngay bên trong môi trường Antigravity.",
  },
  {
    id: "claude-sonnet-5-5-thinking",
    name: "Claude Sonnet 5.5 (Thinking)",
    badge: "Đối tác trong AGY",
    defaultEffort: "high",
    efforts: ["high"],
    desc: "Chạy Claude Sonnet 5.5 có tư duy bên trong môi trường Antigravity.",
  },
];

export function loadCodexModelsFromCache() {
  const codexHome = process.env.CODEX_HOME || path.join(HOME, ".codex");
  const cacheFile = path.join(codexHome, "models_cache.json");
  try {
    const raw = JSON.parse(fs.readFileSync(cacheFile, "utf8"));
    const list = Array.isArray(raw?.models) ? raw.models : [];
    const parsed = list
      .filter((m) => m && (m.slug || m.id) && (m.visibility ?? "list") === "list")
      .map((m, idx) => {
        const id = String(m.slug || m.id);
        const efforts = (m.supported_reasoning_levels || m.supported_reasoning_efforts || [])
          .map((e) => String(e?.effort ?? e))
          .filter(Boolean);
        return {
          id,
          name: String(m.display_name || id),
          badge: idx === 0 ? "Mới nhất trên máy" : undefined,
          defaultEffort: String(m.default_reasoning_level || (efforts.includes("high") ? "high" : efforts[0] || "medium")),
          efforts: efforts.length ? efforts : ["low", "medium", "high", "xhigh"],
          desc: String(m.description || "Mô hình nạp trực tiếp từ bộ nhớ đệm Codex CLI trên máy."),
        };
      });
    if (parsed.length > 0) return parsed;
  } catch {
    // dùng danh sách dự phòng bên dưới
  }
  return FALLBACK_CODEX_MODELS;
}

/** Gộp kết quả `agy models` trực tiếp nếu máy đã cài Antigravity CLI */
async function loadAntigravityModels(agyBin) {
  if (!agyBin) return DEFAULT_ANTIGRAVITY_MODELS;
  const res = await execCliQuiet(agyBin, ["models"], 6000);
  if (!res.ok || !res.stdout) return DEFAULT_ANTIGRAVITY_MODELS;

  const groups = new Map();
  for (const line of res.stdout.split(/\r?\n/)) {
    const m = line.trim().match(/^([a-z0-9._-]+)\s+(.+)$/i);
    if (!m) continue;
    const [, slug, label] = m;
    const effMatch = slug.match(/^(.*)-(low|medium|high|xhigh|max)$/i);
    if (effMatch) {
      const baseId = effMatch[1];
      const eff = effMatch[2].toLowerCase();
      if (!groups.has(baseId)) {
        groups.set(baseId, {
          id: baseId,
          name: label.replace(/\s*\([^)]+\)\s*$/, "").trim(),
          defaultEffort: "high",
          efforts: [],
          slugPattern: `${baseId}-{effort}`,
          desc: "Mô hình nạp trực tiếp từ lệnh `agy models` trên máy.",
        });
      }
      const g = groups.get(baseId);
      if (!g.efforts.includes(eff)) g.efforts.push(eff);
    } else if (!groups.has(slug)) {
      groups.set(slug, {
        id: slug,
        name: label.trim(),
        defaultEffort: "",
        efforts: [],
        fixedEffortLabel: "Cố định theo mô hình",
        desc: "Mô hình nạp trực tiếp từ lệnh `agy models` trên máy.",
      });
    }
  }
  if (!groups.size) return DEFAULT_ANTIGRAVITY_MODELS;
  const order = ["low", "medium", "high", "xhigh", "max"];
  for (const g of groups.values()) {
    g.efforts.sort((a, b) => order.indexOf(a) - order.indexOf(b));
    if (g.efforts.length && !g.efforts.includes(g.defaultEffort)) {
      g.defaultEffort = g.efforts.includes("high") ? "high" : g.efforts[g.efforts.length - 1];
    }
  }
  return [...groups.values()];
}

/** Tính slug thực tế cho Antigravity CLI */
export function resolveAntigravityModelSlug(modelId, effort, models = DEFAULT_ANTIGRAVITY_MODELS) {
  if (!modelId) return "";
  const found = models.find((m) => m.id === modelId);
  if (found?.slugPattern && effort && found.efforts?.includes(effort)) {
    return found.slugPattern.replace("{effort}", effort);
  }
  return modelId;
}

let cachedProbe = null;
let cachedAt = 0;

export async function getProvidersCatalog({ force = false, settings = {} } = {}) {
  if (!force && cachedProbe && Date.now() - cachedAt < 15_000) {
    return cachedProbe;
  }

  const claudeBin = resolveCliBinary("claude");
  const codexBin = resolveCliBinary("codex");
  const agyBin = resolveCliBinary("agy");
  const agyDesktop = resolveAntigravityDesktopApp();

  const [claudeVer, codexVer, agyVer, agyModels] = await Promise.all([
    claudeBin ? execCliQuiet(claudeBin, ["--version"], 5000) : Promise.resolve({ ok: false, stdout: "" }),
    codexBin ? execCliQuiet(codexBin, ["--version"], 5000) : Promise.resolve({ ok: false, stdout: "" }),
    agyBin ? execCliQuiet(agyBin, ["--version"], 5000) : Promise.resolve({ ok: false, stdout: "" }),
    loadAntigravityModels(agyBin),
  ]);

  const lsConfigured = Boolean(
    (settings?.providerPrefs?.antigravity?.lsAddress || process.env.ANTIGRAVITY_LS_ADDRESS) &&
    (settings?.providerPrefs?.antigravity?.csrfToken || process.env.ANTIGRAVITY_CSRF_TOKEN)
  );

  const providers = [
    {
      id: "claude",
      name: "Claude Code",
      vendor: "Anthropic",
      accent: "#f97316",
      skillPrefix: "/",
      installed: Boolean(claudeBin),
      readyToRun: Boolean(claudeBin),
      version: claudeVer.stdout.split(/\r?\n/)[0] || "",
      binaryPath: claudeBin?.rawPath || "",
      bestFor: "Làm trọn gói từ thu tin → kiểm chứng → giọng VieNeu → render → tự mở tờ ảnh .sheet.jpg soát lỗi",
      installHint: "npm install -g @anthropic-ai/claude-code",
      models: CLAUDE_MODELS,
    },
    {
      id: "codex",
      name: "OpenAI Codex",
      vendor: "OpenAI",
      accent: "#10b981",
      skillPrefix: "$",
      installed: Boolean(codexBin),
      readyToRun: Boolean(codexBin),
      version: codexVer.stdout.split(/\r?\n/)[0] || "",
      binaryPath: codexBin?.rawPath || "",
      bestFor: "Sửa/nâng cấp code engine (_shared/), dựng cảnh mới, chạy hàng loạt nhiều video tốc độ cao",
      installHint: "npm install -g @openai/codex",
      models: loadCodexModelsFromCache(),
    },
    {
      id: "antigravity",
      name: "Google Antigravity",
      vendor: "Google DeepMind",
      accent: "#38bdf8",
      skillPrefix: "/",
      installed: Boolean(agyBin || agyDesktop || lsConfigured),
      readyToRun: Boolean(agyBin || lsConfigured),
      hasCli: Boolean(agyBin),
      hasDesktop: Boolean(agyDesktop),
      hasLanguageServer: lsConfigured,
      version: agyVer.stdout.split(/\r?\n/)[0] || (agyDesktop ? "Antigravity Desktop App" : lsConfigured ? "Language Server Connected" : ""),
      binaryPath: agyBin?.rawPath || agyDesktop || "",
      bestFor: "Duyệt trực quan với Remotion Studio / HyperFrames preview, so sánh nhiều phương án song song",
      installHint: "Cài `agy` CLI từ Antigravity (Menu → Install CLI `agy`) để chạy headless trực tiếp hoặc bấm Mở trong Antigravity Desktop.",
      models: agyModels,
    },
  ];

  cachedProbe = {
    platform: process.platform,
    arch: process.arch,
    effortsMeta: EFFORT_LABELS,
    providers,
  };
  cachedAt = Date.now();
  return cachedProbe;
}

/** Sinh mảng tham số CLI + chuỗi hiển thị minh bạch cho người dùng kiểm tra */
export function buildInvocationPreview({
  providerId,
  model,
  effort,
  prompt,
  skillRoot,
  prefs = {},
  agyModels = DEFAULT_ANTIGRAVITY_MODELS,
}) {
  const cleanModel = String(model || "").trim();
  const cleanEffort = String(effort || "").trim();
  const shortPrompt = prompt.length > 90 ? `${prompt.slice(0, 87)}...` : prompt;

  if (providerId === "claude") {
    const perm = prefs.permissionMode || "bypassPermissions";
    const args = [
      "-p",
      prompt,
      "--output-format",
      "stream-json",
      "--verbose",
      ...(cleanModel ? ["--model", cleanModel] : []),
      ...(cleanEffort ? ["--effort", cleanEffort] : []),
      "--permission-mode",
      perm,
      ...(skillRoot ? ["--add-dir", skillRoot] : []),
    ];
    const display = [
      "claude",
      "-p",
      JSON.stringify(shortPrompt),
      ...(cleanModel ? ["--model", cleanModel] : []),
      ...(cleanEffort ? ["--effort", cleanEffort] : []),
      "--permission-mode",
      perm,
    ].join(" ");
    return { args, display };
  }

  if (providerId === "codex") {
    const sandbox = prefs.sandbox || "danger-full-access";
    const args = [
      "exec",
      "--json",
      "--skip-git-repo-check",
      "--sandbox",
      sandbox,
      ...(skillRoot ? ["--add-dir", skillRoot] : []),
      ...(cleanModel ? ["-m", cleanModel] : []),
      ...(cleanEffort ? ["-c", `model_reasoning_effort="${cleanEffort}"`] : []),
      "--",
      prompt,
    ];
    const display = [
      "codex exec --json",
      `--sandbox ${sandbox}`,
      ...(cleanModel ? [`-m ${cleanModel}`] : []),
      ...(cleanEffort ? [`-c model_reasoning_effort="${cleanEffort}"`] : []),
      "--",
      JSON.stringify(shortPrompt),
    ].join(" ");
    return { args, display };
  }

  // Antigravity (`agy`)
  const agySlug = resolveAntigravityModelSlug(cleanModel, cleanEffort, agyModels);
  const skipPerm = prefs.permissionMode !== "auto";
  const args = [
    "-p",
    prompt,
    "--output-format",
    "stream-json",
    ...(agySlug ? ["--model", agySlug] : []),
    ...(cleanEffort ? ["--effort", cleanEffort] : []),
    ...(skipPerm ? ["--dangerously-skip-permissions"] : ["--mode", "auto"]),
    ...(skillRoot ? ["--add-dir", skillRoot] : []),
  ];
  const display = [
    "agy",
    "-p",
    JSON.stringify(shortPrompt),
    ...(agySlug ? ["--model", agySlug] : []),
    ...(cleanEffort ? ["--effort", cleanEffort] : []),
    skipPerm ? "--dangerously-skip-permissions" : "--mode auto",
  ].join(" ");
  return { args, display, agySlug };
}

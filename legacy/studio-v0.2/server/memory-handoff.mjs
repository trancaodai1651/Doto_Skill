// Quản lý Bộ nhớ chung (MEMORY.md) và Sổ bàn giao liên phiên (HANDOFF.md + AGENTS.md + CLAUDE.md + GEMINI.md)
// dùng chung xuyên suốt giữa 3 AI: Google Antigravity, Claude Code và OpenAI Codex.
import fs from "node:fs";
import path from "node:path";

export const MANAGED_DOC_FILES = [
  {
    name: "MEMORY.md",
    roleVi: "Bộ nhớ dài hạn dùng chung (Quyết định, quy ước, sở thích & bẫy kỹ thuật)",
    roleEn: "Shared long-term memory (Decisions, conventions, preferences & pitfalls)",
    agents: ["antigravity", "claude", "codex"],
  },
  {
    name: "HANDOFF.md",
    roleVi: "Sổ bàn giao phiên làm việc (Trạng thái hiện tại, sản phẩm vừa xuất & nhật ký liên AI)",
    roleEn: "Cross-session handoff log (Current state, recent outputs & multi-AI journal)",
    agents: ["antigravity", "claude", "codex"],
  },
  {
    name: "AGENTS.md",
    roleVi: "Quy chuẩn điều phối chung (Codex CLI & Antigravity tự động nạp đầu phiên)",
    roleEn: "Universal agent rules (Auto-loaded by Codex CLI & Antigravity)",
    agents: ["antigravity", "claude", "codex"],
  },
  {
    name: "CLAUDE.md",
    roleVi: "Điểm neo ngữ cảnh tự động cho Claude Code (@AGENTS.md + MEMORY/HANDOFF)",
    roleEn: "Auto-loaded context anchor for Claude Code (@AGENTS.md + MEMORY/HANDOFF)",
    agents: ["claude"],
  },
  {
    name: "GEMINI.md",
    roleVi: "Điểm neo ngữ cảnh tự động cho Google Antigravity / Gemini CLI",
    roleEn: "Auto-loaded context anchor for Google Antigravity / Gemini CLI",
    agents: ["antigravity"],
  },
];

function todayIso() {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function defaultTemplates(workspaceDir) {
  const today = todayIso();
  return {
    "MEMORY.md": `# MEMORY.md — Bộ nhớ chung cho mọi Agent (Antigravity · Claude Code · Codex)

> Trí nhớ dài hạn DÙNG CHUNG tại \`${workspaceDir}\`.
> Mọi AI (**Google Antigravity**, **Claude Code**, **OpenAI Codex**) đều đọc tệp này đầu phiên và ghi nối tiếp khi có quy ước, quyết định kiến trúc hoặc bẫy mới.
> Trạng thái công việc đang dở dang ghi ở \`HANDOFF.md\`.

## 1. Mục tiêu & Sở thích người dùng
- Sản xuất video tự động chất lượng cao, phụ đề khớp giọng đọc, ghi nguồn minh bạch trên hình, che mặt bắt buộc với hồ sơ vụ án.
- Chạy linh hoạt song song nhiều phiên trên cùng 1 AI hoặc phối hợp nhiều AI (**Antigravity**, **Claude Code**, **Codex**).

## 2. Kiến trúc & Quy ước dùng chung
- ${today} · Mọi Skill đặt tại thư mục gốc hoặc thư mục lưu tuỳ chỉnh được đồng bộ qua Doto Skill Studio.
- ${today} · Trước khi kết thúc phiên, luôn kiểm định đầu ra (\`measure.mjs\`, \`.sheet.jpg\`, \`qc-sync.mjs\`) và cập nhật \`HANDOFF.md\`.

## 3. Bẫy kỹ thuật cần tránh
- Luôn bọc ngoặc kép đường dẫn có khoảng trắng trên Windows khi gọi \`ffmpeg\` / \`npx remotion\`.
`,
    "HANDOFF.md": `# HANDOFF.md — Bàn giao liên phiên (Antigravity · Claude Code · Codex)

> Cập nhật tệp này cuối mỗi phiên làm việc để AI tiếp theo (dù là **Antigravity**, **Claude Code** hay **Codex**) nối tiếp ngay lập tức mà không mất ngữ cảnh.

## Trạng thái hiện tại (${today})
- Bộ nhớ chung (\`MEMORY.md\`) và quy chuẩn điều phối (\`AGENTS.md\`, \`CLAUDE.md\`, \`GEMINI.md\`) đã sẵn sàng cho cả 3 AI.
- Sẵn sàng nhận các phiên dựng video đơn lẻ hoặc đa phiên song song từ Doto Skill Studio.

## Việc tiếp theo / Đang mở
- [ ] Duyệt các bản nháp video mới xuất và kiểm tra tờ ảnh \`.sheet.jpg\`.

## Nhật ký phiên làm việc (Mới nhất ở trên)
- ${today} · Doto Skill Studio · Khởi tạo / đồng bộ bộ tệp bàn giao liên AI cho Antigravity, Claude Code và Codex.
`,
    "AGENTS.md": `# Quy chuẩn Điều phối Đa AI (Google Antigravity · Claude Code · OpenAI Codex)

> **Đọc bắt buộc đầu phiên:**
> 1. Đọc [MEMORY.md](./MEMORY.md) để nắm quy ước dài hạn, quyết định kiến trúc và các bẫy kỹ thuật đã biết.
> 2. Đọc [HANDOFF.md](./HANDOFF.md) để biết trạng thái hiện tại, phiên trước vừa làm gì và việc nào đang dở dang.
> 3. Khi hoàn thành tác vụ, **cập nhật lại \`HANDOFF.md\`** (và ghi thêm vào \`MEMORY.md\` nếu phát hiện quy ước/bẫy mới) để bất kỳ Agent nào vào sau đều tiếp quản mượt mà.

## Cú pháp gọi Skill theo từng AI
- **Google Antigravity**: \`/<ten-skill> <yêu cầu>\` (đọc \`GEMINI.md\`, \`AGENTS.md\`, \`SKILL.md\`).
- **Claude Code**: \`/<ten-skill> <yêu cầu>\` (đọc \`CLAUDE.md\` → \`@AGENTS.md\`, \`SKILL.md\`).
- **OpenAI Codex**: \`$<ten-skill> <yêu cầu>\` (đọc \`AGENTS.md\`, \`SKILL.md\`).
`,
    "CLAUDE.md": `@AGENTS.md
@MEMORY.md
@HANDOFF.md
`,
    "GEMINI.md": `# Google Antigravity — Ngữ cảnh Dùng chung với Claude Code & Codex

Trước khi thực hiện bất kỳ yêu cầu nào trong không gian làm việc này:
1. Đọc \`AGENTS.md\` để nắm quy tắc chung của hệ thống Skill.
2. Đọc \`MEMORY.md\` (bộ nhớ dài hạn dùng chung) và \`HANDOFF.md\` (trạng thái bàn giao mới nhất từ phiên trước của Antigravity / Claude Code / Codex).
3. Sau khi hoàn tất công việc, cập nhật \`HANDOFF.md\` (mục Trạng thái hiện tại & Nhật ký) và bổ sung bài học mới vào \`MEMORY.md\` nếu có.
`,
  };
}

export function getMemoryHandoffStatus(workspaceDir) {
  if (!workspaceDir) return { workspaceDir: "", exists: false, files: [] };
  const resolved = path.resolve(workspaceDir);
  const exists = fs.existsSync(resolved);
  const files = MANAGED_DOC_FILES.map((meta) => {
    const fullPath = path.join(resolved, meta.name);
    let fileExists = false;
    let size = 0;
    let mtimeMs = 0;
    let preview = "";
    try {
      if (fs.existsSync(fullPath) && fs.statSync(fullPath).isFile()) {
        const st = fs.statSync(fullPath);
        fileExists = true;
        size = st.size;
        mtimeMs = st.mtimeMs;
        preview = fs.readFileSync(fullPath, "utf8").slice(0, 400);
      }
    } catch {
      // ignore
    }
    return {
      ...meta,
      path: fullPath,
      exists: fileExists,
      size,
      mtimeMs,
      preview,
    };
  });
  return { workspaceDir: resolved, exists, files };
}

export function initOrSyncMemoryHandoff(workspaceDir, { overwrite = false } = {}) {
  if (!workspaceDir) throw new Error("Thiếu đường dẫn thư mục làm việc.");
  const resolved = path.resolve(workspaceDir);
  fs.mkdirSync(resolved, { recursive: true });
  const templates = defaultTemplates(resolved);
  const created = [];
  const kept = [];

  for (const meta of MANAGED_DOC_FILES) {
    const fullPath = path.join(resolved, meta.name);
    if (!fs.existsSync(fullPath) || overwrite) {
      fs.writeFileSync(fullPath, templates[meta.name], "utf8");
      created.push(meta.name);
    } else {
      kept.push(meta.name);
    }
  }

  return {
    ok: true,
    workspaceDir: resolved,
    created,
    kept,
    status: getMemoryHandoffStatus(resolved),
  };
}

export function readMemoryDoc(workspaceDir, fileName) {
  const allowed = MANAGED_DOC_FILES.some((f) => f.name === fileName);
  if (!allowed) throw new Error(`Tệp không nằm trong danh mục bộ nhớ/bàn giao: ${fileName}`);
  const fullPath = path.join(path.resolve(workspaceDir), fileName);
  if (!fs.existsSync(fullPath)) {
    return { ok: true, exists: false, fileName, path: fullPath, content: "" };
  }
  return {
    ok: true,
    exists: true,
    fileName,
    path: fullPath,
    content: fs.readFileSync(fullPath, "utf8"),
  };
}

export function writeMemoryDoc(workspaceDir, fileName, content) {
  const allowed = MANAGED_DOC_FILES.some((f) => f.name === fileName);
  if (!allowed) throw new Error(`Tệp không hợp lệ: ${fileName}`);
  const resolved = path.resolve(workspaceDir);
  fs.mkdirSync(resolved, { recursive: true });
  const fullPath = path.join(resolved, fileName);
  fs.writeFileSync(fullPath, String(content ?? ""), "utf8");
  return { ok: true, fileName, path: fullPath };
}

export function appendMemoryEntry(workspaceDir, { agent = "Doto Studio", note = "" }) {
  const clean = String(note || "").trim();
  if (!clean) throw new Error("Nội dung ghi chú bộ nhớ chung không được để trống.");
  initOrSyncMemoryHandoff(workspaceDir, { overwrite: false });
  const fullPath = path.join(path.resolve(workspaceDir), "MEMORY.md");
  const cur = fs.readFileSync(fullPath, "utf8");
  const line = `- ${todayIso()} · [${agent}] · ${clean}\n`;
  fs.writeFileSync(fullPath, `${cur.trimEnd()}\n${line}`, "utf8");
  return { ok: true, path: fullPath };
}

export function appendHandoffSession(workspaceDir, job) {
  if (!workspaceDir || !job) return { ok: false };
  try {
    initOrSyncMemoryHandoff(workspaceDir, { overwrite: false });
    const fullPath = path.join(path.resolve(workspaceDir), "HANDOFF.md");
    const cur = fs.readFileSync(fullPath, "utf8");
    const stamp = new Date().toLocaleString("vi-VN");
    const statusLabel = job.status === "completed" ? "✓ Hoàn tất" : `✗ ${job.status}`;
    const summarySnippet = String(job.resultText || job.error || job.prompt || "")
      .replace(/\s+/g, " ")
      .slice(0, 240);
    const logLine = `- ${todayIso()} (${stamp}) · **${String(job.providerId || "").toUpperCase()}** (\`${job.model || "default"}${job.effort ? `/${job.effort}` : ""}\`) · Skill \`${job.skillId}\`${job.projectFolder ? ` → \`${job.projectFolder}\`` : ""} · ${statusLabel}: ${summarySnippet}`;

    let updated;
    if (/^##\s+Nhật ký.*$/m.test(cur)) {
      updated = cur.replace(/^(##\s+Nhật ký.*)$/m, `$1\n${logLine}`);
    } else {
      updated = `${cur.trimEnd()}\n\n## Nhật ký\n${logLine}\n`;
    }
    fs.writeFileSync(fullPath, updated, "utf8");
    return { ok: true, path: fullPath };
  } catch {
    return { ok: false };
  }
}

export function buildSharedMemoryPromptHint(workspaceDir) {
  if (!workspaceDir) return "";
  const norm = workspaceDir.replace(/\\/g, "/");
  return `Đọc bộ nhớ chung "${norm}/MEMORY.md" và sổ bàn giao "${norm}/HANDOFF.md" trước khi làm; khi xong cập nhật kết quả vào HANDOFF.md để Antigravity, Claude và Codex phối hợp liền mạch.`;
}

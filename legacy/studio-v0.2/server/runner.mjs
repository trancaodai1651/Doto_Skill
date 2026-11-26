// Bộ điều phối tiến trình chạy việc trực tiếp & song song nhiều phiên trên cùng 1 AI hoặc đồng thời nhiều AI
// (Google Antigravity · Claude Code · OpenAI Codex), hỗ trợ cả CLI stream-json & Language Server API.
import { spawn } from "node:child_process";
import fs from "node:fs";
import {
  resolveCliBinary,
  resolveAntigravityDesktopApp,
  standaloneEnvironment,
} from "./cli-resolver.mjs";
import {
  buildInvocationPreview,
  DEFAULT_ANTIGRAVITY_MODELS,
  resolveAntigravityModelSlug,
} from "./providers.mjs";
import { loadJobsHistory, saveJobsHistory } from "./data.mjs";

const MAX_LOG_ENTRIES = 250;

export function createJobRunner({ onEvent, onJobDone }) {
  const jobs = new Map();
  for (const old of loadJobsHistory()) {
    if (old.status === "running") {
      old.status = "interrupted";
      old.error = "Ứng dụng đã khởi động lại khi phiên trước chưa kết thúc.";
    }
    jobs.set(old.id, { ...old, proc: null });
  }

  function serializeJob(j) {
    const { proc: _proc, buf: _buf, ...rest } = j;
    return rest;
  }

  function persist() {
    const all = [...jobs.values()]
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, 100)
      .map(serializeJob);
    saveJobsHistory(all);
  }

  function emit(type, job, extra = {}) {
    onEvent?.({ type, job: serializeJob(job), ...extra });
  }

  function pushLog(job, kind, message, detail = "") {
    const entry = {
      at: Date.now(),
      kind,
      message: String(message || "").slice(0, 1200),
      detail: detail ? String(detail).slice(0, 800) : "",
    };
    job.logs.push(entry);
    if (job.logs.length > MAX_LOG_ENTRIES) {
      job.logs.splice(0, job.logs.length - MAX_LOG_ENTRIES);
    }
    job.updatedAt = Date.now();
    emit("job:log", job, { entry });
  }

  function finishJob(job, status, errorMsg = "") {
    if (job.finishedAt) return;
    job.status = status;
    if (errorMsg) job.error = String(errorMsg).slice(0, 600);
    job.finishedAt = Date.now();
    job.updatedAt = job.finishedAt;
    job.proc = null;
    persist();
    const clean = serializeJob(job);
    emit("job:done", job);
    try {
      onJobDone?.(clean);
    } catch {
      // ignore hook errors
    }
  }

  async function runViaAntigravityLanguageServer(job, { lsAddress, csrfToken }) {
    const baseUrl = /^https?:\/\//i.test(lsAddress) ? lsAddress : `http://${lsAddress}`;
    const slug = resolveAntigravityModelSlug(job.model, job.effort, DEFAULT_ANTIGRAVITY_MODELS) || "pro";
    pushLog(job, "system", `Kết nối Antigravity Language Server (${baseUrl}) với model ${slug}...`);

    try {
      const resp = await fetch(`${baseUrl}/product.v1.ConversationService/CreateConversation`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Connect-Protocol-Version": "1",
          "x-antigravity-csrf-token": csrfToken,
        },
        body: JSON.stringify({
          title: `[Doto Skill] ${job.sessionLabel || job.skillId} (${job.model || "auto"})`,
          model: slug,
          tags: ["doto-skill", job.skillId, ...(job.batchId ? [job.batchId] : [])],
          prompt: [{ text: job.prompt }],
        }),
      });

      if (!resp.ok) {
        const txt = await resp.text();
        finishJob(job, "failed", `Language Server trả về HTTP ${resp.status}: ${txt.slice(0, 240)}`);
        return;
      }

      const data = await resp.json();
      const convId = data?.conversation?.id || "";
      job.sessionId = convId;
      pushLog(job, "tool", "Đã khởi tạo hội thoại song song trong Antigravity", convId ? `ID: ${convId}` : "");
      persist();

      if (!convId) {
        job.resultText = "Đã gửi yêu cầu sang Antigravity thành công.";
        finishJob(job, "completed");
        return;
      }

      const poll = async () => {
        if (job.status !== "running") return;
        try {
          const r = await fetch(`${baseUrl}/product.v1.ConversationService/GetConversation`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "Connect-Protocol-Version": "1",
              "x-antigravity-csrf-token": csrfToken,
            },
            body: JSON.stringify({ conversationId: convId }),
          });
          if (r.ok) {
            const cj = await r.json();
            const runState = cj?.state?.runState || cj?.conversation?.state?.runState || "";
            const summary = cj?.state?.summary || cj?.conversation?.state?.summary || "";
            if (summary && summary !== job.lastSummary) {
              job.lastSummary = summary;
              pushLog(job, "assistant", summary);
            }
            if (runState === "RUN_STATE_IDLE") {
              job.resultText = summary || `Phiên Antigravity (${convId}) đã hoàn tất.`;
              finishJob(job, "completed");
              return;
            }
            if (runState === "RUN_STATE_CANCELLING") {
              finishJob(job, "stopped", "Phiên Antigravity đã bị huỷ.");
              return;
            }
          }
        } catch {
          // tiếp tục chờ
        }
        setTimeout(poll, 3500);
      };
      setTimeout(poll, 3000);
    } catch (err) {
      finishJob(job, "failed", `Không kết nối được Antigravity Language Server: ${err?.message || err}`);
    }
  }

  function parseStreamLine(job, line) {
    let j;
    try {
      j = JSON.parse(line);
    } catch {
      if (line.trim()) pushLog(job, "stdout", line.trim());
      return;
    }

    if (job.providerId === "claude") {
      if (j.type === "system" && j.subtype === "init" && j.session_id) {
        job.sessionId = j.session_id;
        pushLog(job, "system", `Phiên Claude đã khởi tạo (${j.model || job.model || "default"})`, `Session: ${j.session_id}`);
      } else if (j.type === "assistant" && j.message?.content) {
        for (const block of j.message.content) {
          if (block.type === "tool_use") {
            const detail =
              block.input?.command ||
              block.input?.file_path ||
              block.input?.pattern ||
              block.input?.query ||
              JSON.stringify(block.input || {}).slice(0, 180);
            pushLog(job, "tool", `Công cụ: ${block.name}`, detail);
          } else if (block.type === "text" && block.text?.trim()) {
            job.resultText = block.text.trim();
            pushLog(job, "assistant", block.text.trim());
          }
        }
      } else if (j.type === "result") {
        if (j.result) job.resultText = String(j.result);
        if (j.usage) job.usage = j.usage;
        if (j.is_error) {
          job.error = String(j.result || j.subtype || "Lỗi từ Claude Code");
          pushLog(job, "error", job.error);
        } else if (j.result) {
          pushLog(job, "result", "Hoàn tất phản hồi cuối.");
        }
      }
      return;
    }

    if (job.providerId === "codex") {
      if (j.type === "thread.started" && j.thread_id) {
        job.sessionId = j.thread_id;
        pushLog(job, "system", `Phiên Codex đã khởi tạo (${job.model || "default"})`, `Thread: ${j.thread_id}`);
      } else if (j.type === "item.started" && j.item) {
        if (j.item.type === "command_execution") {
          pushLog(job, "tool", "Chạy lệnh terminal", j.item.command || "");
        } else if (j.item.type === "web_search") {
          pushLog(job, "tool", "Tìm kiếm nguồn web", j.item.query || "");
        }
      } else if (j.type === "item.completed" && j.item) {
        if (j.item.type === "agent_message" && j.item.text) {
          job.resultText = String(j.item.text);
          pushLog(job, "assistant", j.item.text);
        } else if (j.item.type === "file_change") {
          const files = (j.item.changes || []).map((c) => `${c.kind}: ${c.path}`).join(", ");
          pushLog(job, "tool", "Cập nhật tệp dự án", files);
        }
      } else if (j.type === "turn.completed" && j.usage) {
        job.usage = j.usage;
      } else if (j.type === "turn.failed" || j.type === "error") {
        const msg = j.error?.message || j.message || "Lỗi thực thi Codex";
        job.error = String(msg);
        pushLog(job, "error", job.error);
      }
      return;
    }

    // Antigravity (`agy --output-format stream-json`)
    if (j.event === "init") {
      if (j.conversation_id) job.sessionId = j.conversation_id;
      pushLog(job, "system", `Phiên Antigravity CLI đã khởi tạo`, j.conversation_id ? `Conversation: ${j.conversation_id}` : "");
    } else if (j.event === "step_update" && j.step_update) {
      const su = j.step_update;
      if (su.step_type === "tool" && su.state === "DONE") {
        const cmd = su.tool_info?.parameters?.CommandLine || su.tool_info?.parameters?.Path || "";
        pushLog(job, "tool", `Công cụ: ${su.tool_name || su.tool_info?.name || "tool"}`, cmd);
      } else if (su.step_type === "agent_response" && su.text_delta && su.state === "DONE") {
        job.resultText = `${job.resultText || ""}${su.text_delta}`.trim();
        pushLog(job, "assistant", su.text_delta.trim());
      }
    } else if (j.event === "result" && j.result) {
      const r = j.result;
      if (r.conversation_id) job.sessionId = r.conversation_id;
      if (r.usage) job.usage = r.usage;
      if (r.response) job.resultText = String(r.response).trim();
      if (r.status && r.status !== "SUCCESS") {
        job.error = String(r.error || `Trạng thái: ${r.status}`);
        pushLog(job, "error", job.error);
      }
    }
  }

  function startJob({
    skillId,
    skillTitle,
    providerId,
    model,
    effort,
    prompt,
    skillRoot,
    extraSkillDirs = [],
    projectFolder = "",
    outputDir = "",
    batchId = "",
    sessionLabel = "",
    prefs = {},
    agyModels = DEFAULT_ANTIGRAVITY_MODELS,
    resumeSessionId = "",
  }) {
    if (!prompt || !String(prompt).trim()) {
      return { ok: false, error: "Câu lệnh yêu cầu tạo video không được để trống." };
    }
    if (!skillRoot || !fs.existsSync(skillRoot)) {
      return { ok: false, error: `Không tìm thấy thư mục bộ Skill: ${skillRoot}` };
    }

    const cliName = providerId === "antigravity" ? "agy" : providerId;
    const bin = resolveCliBinary(cliName);
    const lsAddress = prefs.lsAddress || process.env.ANTIGRAVITY_LS_ADDRESS || "";
    const csrfToken = prefs.csrfToken || process.env.ANTIGRAVITY_CSRF_TOKEN || "";

    if (!bin && !(providerId === "antigravity" && lsAddress && csrfToken)) {
      const desktopApp = providerId === "antigravity" ? resolveAntigravityDesktopApp() : null;
      return {
        ok: false,
        code: "CLI_NOT_FOUND",
        hasDesktopApp: Boolean(desktopApp),
        error:
          providerId === "antigravity"
            ? "Máy chưa cài lệnh `agy` CLI trong PATH (hiện mới có Antigravity Desktop). Bạn có thể cài `agy` CLI trong Antigravity, nhập Language Server Address hoặc bấm nút \"Mở trong Antigravity Desktop\"."
            : `Không tìm thấy công cụ dòng lệnh \`${cliName}\` trên máy. Hãy kiểm tra cài đặt ${providerId}.`,
      };
    }

    const id = `job_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;
    const preview = buildInvocationPreview({
      providerId,
      model,
      effort,
      prompt,
      skillRoot,
      prefs,
      agyModels,
    });

    const job = {
      id,
      batchId: batchId || "",
      sessionLabel: sessionLabel || "",
      skillId,
      skillTitle: skillTitle || skillId,
      providerId,
      model: model || "",
      effort: effort || "",
      prompt: String(prompt).trim(),
      commandDisplay: preview.display,
      skillRoot,
      projectFolder: projectFolder || "",
      outputDir: outputDir || "",
      status: "running",
      sessionId: resumeSessionId || "",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      finishedAt: null,
      resultText: "",
      error: "",
      usage: null,
      logs: [],
      buf: "",
      proc: null,
    };

    jobs.set(id, job);
    persist();
    emit("job:started", job);

    if (!bin && providerId === "antigravity" && lsAddress && csrfToken) {
      runViaAntigravityLanguageServer(job, { lsAddress, csrfToken });
      return { ok: true, job: serializeJob(job) };
    }

    let cliArgs = [...preview.args];
    let stdinPayload = null;

    // Cho phép truy cập thêm thư mục lưu video tuỳ chỉnh hoặc thư mục skill phụ
    const extraDirsToAllow = [...new Set([outputDir, ...extraSkillDirs].filter((d) => d && fs.existsSync(d)))];

    if (providerId === "claude") {
      for (const d of extraDirsToAllow) cliArgs.push("--add-dir", d);
      if (resumeSessionId) cliArgs.push("--resume", resumeSessionId);
      if (job.prompt.length > 4000) {
        cliArgs[1] = "Thực hiện đầy đủ yêu cầu dựng video được truyền qua stdin.";
        stdinPayload = job.prompt;
      }
    } else if (providerId === "codex") {
      const sepIdx = cliArgs.indexOf("--");
      if (sepIdx > 0) {
        const inject = [];
        for (const d of extraDirsToAllow) inject.push("--add-dir", d);
        if (resumeSessionId) inject.push("resume", resumeSessionId);
        if (inject.length) cliArgs.splice(sepIdx, 0, ...inject);
      }
      const newSepIdx = cliArgs.indexOf("--");
      if (job.prompt.length > 4000 && newSepIdx >= 0) {
        cliArgs[cliArgs.length - 1] = "-";
        stdinPayload = job.prompt;
      }
    } else if (providerId === "antigravity") {
      for (const d of extraDirsToAllow) cliArgs.push("--add-dir", d);
      if (resumeSessionId) cliArgs.push("--conversation", resumeSessionId);
    }

    pushLog(
      job,
      "system",
      `Khởi chạy phiên ${sessionLabel ? `[${sessionLabel}] ` : ""}${providerId.toUpperCase()} tại ${skillRoot}`,
      preview.display
    );

    const env = standaloneEnvironment({
      VIENEU_PYTHON:
        process.platform === "win32"
          ? `${skillRoot}\\_tools\\VieNeu-TTS\\.venv\\Scripts\\python.exe`
          : `${skillRoot}/_tools/VieNeu-TTS/.venv/bin/python`,
    });

    let proc;
    try {
      proc = spawn(bin.file, [...bin.prefixArgs, ...cliArgs], {
        cwd: skillRoot,
        env,
        stdio: [stdinPayload ? "pipe" : "ignore", "pipe", "pipe"],
        detached: process.platform !== "win32",
        windowsHide: true,
      });
    } catch (err) {
      finishJob(job, "failed", `Lỗi khởi chạy tiến trình: ${err?.message || err}`);
      return { ok: false, error: job.error };
    }

    job.proc = proc;
    job.pid = proc.pid;

    if (stdinPayload && proc.stdin) {
      proc.stdin.on("error", () => {});
      proc.stdin.end(stdinPayload, "utf8");
    }

    proc.stdout.on("data", (chunk) => {
      job.buf += chunk.toString("utf8");
      let nl;
      while ((nl = job.buf.indexOf("\n")) >= 0) {
        const line = job.buf.slice(0, nl).replace(/\r$/, "");
        job.buf = job.buf.slice(nl + 1);
        if (line) parseStreamLine(job, line);
      }
      if (job.buf.length > 2_000_000) job.buf = "";
    });

    proc.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf8").trim();
      if (!text) return;
      for (const line of text.split(/\r?\n/).slice(-4)) {
        if (line.trim()) pushLog(job, "stderr", line.trim());
      }
    });

    proc.on("error", (err) => {
      finishJob(job, "failed", err?.message || String(err));
    });

    proc.on("close", (code) => {
      if (job.buf.trim()) {
        parseStreamLine(job, job.buf.trim());
        job.buf = "";
      }
      if (job.status === "stopped") {
        finishJob(job, "stopped");
      } else if (code === 0 && !job.error) {
        finishJob(job, "completed");
      } else {
        finishJob(job, "failed", job.error || `Tiến trình kết thúc với mã thoát ${code}`);
      }
    });

    return { ok: true, job: serializeJob(job) };
  }

  function stopJob(id) {
    const job = jobs.get(id);
    if (!job || job.status !== "running") return { ok: false, error: "Tiến trình không còn chạy." };
    job.status = "stopped";
    pushLog(job, "system", "Đã nhận lệnh dừng từ người dùng...");

    if (job.proc && job.pid) {
      try {
        if (process.platform === "win32") {
          spawn("taskkill", ["/PID", String(job.pid), "/T", "/F"], { windowsHide: true });
        } else {
          process.kill(-job.pid, "SIGTERM");
        }
      } catch {
        try {
          job.proc.kill();
        } catch {
          // ignore
        }
      }
    }
    finishJob(job, "stopped", "Đã dừng bởi người dùng.");
    return { ok: true, job: serializeJob(job) };
  }

  function stopAllJobs() {
    const stoppedIds = [];
    for (const j of jobs.values()) {
      if (j.status === "running") {
        stopJob(j.id);
        stoppedIds.push(j.id);
      }
    }
    return { ok: true, stoppedIds };
  }

  function clearFinishedJobs() {
    for (const [id, j] of jobs.entries()) {
      if (j.status !== "running") {
        jobs.delete(id);
      }
    }
    persist();
    onEvent?.({ type: "jobs:cleared", jobs: listJobs() });
    return { ok: true, jobs: listJobs() };
  }

  function listJobs() {
    return [...jobs.values()]
      .sort((a, b) => b.createdAt - a.createdAt)
      .map(serializeJob);
  }

  function getJob(id) {
    const j = jobs.get(id);
    return j ? serializeJob(j) : null;
  }

  return { startJob, stopJob, stopAllJobs, clearFinishedJobs, listJobs, getJob };
}

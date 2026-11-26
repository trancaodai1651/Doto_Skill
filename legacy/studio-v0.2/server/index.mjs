// Máy chủ hợp nhất Doto Skill Studio v0.2.0 (HTTP + SSE Streaming + Media Byte-Range + Mobile Photos Upload + Cloudflare Tunnel CRUD + Shared Memory).
// Dùng chung cho cả phiên bản Web (PC / iPad / iPhone / Android) và phiên bản Desktop Tauri (Windows + macOS Universal).
import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { loadSettings, saveSettings, getAllowedRoots } from "./data.mjs";
import { discoverSkills, composeSkillPrompt } from "./skills.mjs";
import { getProvidersCatalog, buildInvocationPreview } from "./providers.mjs";
import { listVideoProjects, isAllowedMediaPath } from "./videos.mjs";
import { createJobRunner } from "./runner.mjs";
import { resolveAntigravityDesktopApp } from "./cli-resolver.mjs";
import {
  importSkillFromArchiveFile,
  importSkillFromLocalPath,
  importSkillFromUploadedFiles,
  deleteCustomSkill,
  pickNativePath,
} from "./skill-importer.mjs";
import {
  saveUploadedMediaStream,
  listUploadedMedia,
  deleteUploadedMedia,
} from "./media-upload.mjs";
import { createTunnelManager } from "./tunnels.mjs";
import {
  getMemoryHandoffStatus,
  initOrSyncMemoryHandoff,
  readMemoryDoc,
  writeMemoryDoc,
  appendMemoryEntry,
  appendHandoffSession,
  buildSharedMemoryPromptHint,
} from "./memory-handoff.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "..");
const DIST_DIR = path.join(ROOT, "dist");

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".webmanifest": "application/manifest+json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".heic": "image/heic",
  ".mp4": "video/mp4",
  ".mov": "video/quicktime",
  ".m4v": "video/mp4",
  ".wav": "audio/wav",
  ".mp3": "audio/mpeg",
  ".ico": "image/x-icon",
};

function getLanUrls(port) {
  const urls = [];
  const nets = os.networkInterfaces();
  for (const list of Object.values(nets)) {
    for (const iface of list || []) {
      if (iface && iface.family === "IPv4" && !iface.internal) {
        urls.push(`http://${iface.address}:${port}`);
      }
    }
  }
  return urls;
}

function sendJson(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, X-File-Name, X-Target-Dir, X-Overwrite, X-Sync-Links, X-Skill-Name",
  });
  res.end(body);
}

function readBody(req, maxChars = 35_000_000) {
  return new Promise((resolve, reject) => {
    let raw = "";
    req.on("data", (chunk) => {
      raw += chunk.toString("utf8");
      if (raw.length > maxChars) {
        reject(new Error("Payload vượt quá giới hạn cho phép."));
        req.destroy();
      }
    });
    req.on("end", () => {
      if (!raw.trim()) return resolve({});
      try {
        resolve(JSON.parse(raw));
      } catch (err) {
        reject(err);
      }
    });
    req.on("error", reject);
  });
}

function streamRequestToTempFile(req, ext = ".bin", maxBytes = 600 * 1024 * 1024) {
  return new Promise((resolve, reject) => {
    const tmpFile = path.join(os.tmpdir(), `doto-upload-${Date.now()}-${Math.random().toString(36).slice(2, 7)}${ext}`);
    const ws = fs.createWriteStream(tmpFile);
    let size = 0;
    req.on("data", (chunk) => {
      size += chunk.length;
      if (size > maxBytes) {
        ws.destroy();
        try { fs.unlinkSync(tmpFile); } catch {}
        reject(new Error("Tệp tải lên vượt quá giới hạn 600MB."));
        req.destroy();
      }
    });
    req.pipe(ws);
    ws.on("finish", () => resolve({ tmpFile, size }));
    ws.on("error", (err) => {
      try { fs.unlinkSync(tmpFile); } catch {}
      reject(err);
    });
    req.on("error", (err) => {
      ws.destroy();
      try { fs.unlinkSync(tmpFile); } catch {}
      reject(err);
    });
  });
}

function serveMediaWithRange(req, res, filePath, forceDownload = false) {
  let stat;
  try {
    stat = fs.statSync(filePath);
    if (!stat.isFile()) throw new Error("Not file");
  } catch {
    res.writeHead(404);
    res.end("Not found");
    return;
  }

  const ext = path.extname(filePath).toLowerCase();
  const contentType = MIME[ext] || "application/octet-stream";
  const baseHeaders = {
    "Accept-Ranges": "bytes",
    "Content-Type": contentType,
    "Access-Control-Allow-Origin": "*",
  };
  if (forceDownload) {
    const encoded = encodeURIComponent(path.basename(filePath));
    baseHeaders["Content-Disposition"] = `attachment; filename*=UTF-8''${encoded}`;
  }

  const range = req.headers.range;
  if (range && !forceDownload) {
    const m = range.match(/bytes=(\d*)-(\d*)/);
    if (m) {
      const start = m[1] ? parseInt(m[1], 10) : 0;
      const end = m[2] ? Math.min(stat.size - 1, parseInt(m[2], 10)) : stat.size - 1;
      if (start <= end && start < stat.size) {
        res.writeHead(206, {
          ...baseHeaders,
          "Content-Range": `bytes ${start}-${end}/${stat.size}`,
          "Content-Length": end - start + 1,
        });
        fs.createReadStream(filePath, { start, end }).pipe(res);
        return;
      }
    }
  }

  res.writeHead(200, {
    ...baseHeaders,
    "Content-Length": stat.size,
  });
  fs.createReadStream(filePath).pipe(res);
}

function openInOsExplorer(targetPath) {
  if (!targetPath || !fs.existsSync(targetPath)) return false;
  try {
    if (process.platform === "win32") {
      const isFile = fs.statSync(targetPath).isFile();
      spawn("explorer.exe", isFile ? [`/select,${targetPath}`] : [targetPath], { detached: true, windowsHide: true }).unref();
    } else if (process.platform === "darwin") {
      const isFile = fs.statSync(targetPath).isFile();
      spawn("open", isFile ? ["-R", targetPath] : [targetPath], { detached: true }).unref();
    } else {
      spawn("xdg-open", [fs.statSync(targetPath).isFile() ? path.dirname(targetPath) : targetPath], { detached: true }).unref();
    }
    return true;
  } catch {
    return false;
  }
}

export async function startServer({ port = 4320, host = "127.0.0.1" } = {}) {
  const sseClients = new Set();

  const broadcast = (event) => {
    const msg = `data: ${JSON.stringify(event)}\n\n`;
    for (const client of sseClients) {
      try {
        client.write(msg);
      } catch {
        sseClients.delete(client);
      }
    }
  };

  let activePort = port;

  const runner = createJobRunner({
    onEvent: (ev) => broadcast(ev),
    onJobDone: (job) => {
      const settings = loadSettings();
      if (settings.autoLogHandoff && settings.skillRoot) {
        appendHandoffSession(settings.skillRoot, job);
      }
    },
  });

  let tunnelManager = null;

  const server = http.createServer(async (req, res) => {
    if (req.method === "OPTIONS") {
      res.writeHead(204, {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type, X-File-Name, X-Target-Dir, X-Overwrite, X-Sync-Links, X-Skill-Name",
      });
      res.end();
      return;
    }

    const urlObj = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
    const pathname = urlObj.pathname;

    try {
      if (pathname === "/api/doctor" && req.method === "GET") {
        sendJson(res, 200, {
          ok: true,
          service: "doto-skill",
          version: "0.2.0",
          platform: process.platform,
          arch: process.arch,
          port: activePort,
          lanUrls: getLanUrls(activePort),
        });
        return;
      }

      if (pathname === "/api/events" && req.method === "GET") {
        res.writeHead(200, {
          "Content-Type": "text/event-stream; charset=utf-8",
          "Cache-Control": "no-cache, no-transform",
          Connection: "keep-alive",
          "Access-Control-Allow-Origin": "*",
        });
        res.write(`data: ${JSON.stringify({ type: "connected", at: Date.now() })}\n\n`);
        sseClients.add(res);
        req.on("close", () => sseClients.delete(res));
        return;
      }

      if (pathname === "/api/bootstrap" && req.method === "GET") {
        const force = urlObj.searchParams.get("refresh") === "1";
        const settings = loadSettings();
        const extraSkillRoots = [settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])];
        const [providersInfo, skillsInfo, tunnelsOverview] = await Promise.all([
          getProvidersCatalog({ force, settings }),
          Promise.resolve(discoverSkills(settings.skillRoot, extraSkillRoots)),
          tunnelManager.getOverview(),
        ]);
        const videos = listVideoProjects(settings.skillRoot, [settings.videoOutputDir]);
        const uploadedMedia = listUploadedMedia(settings.mediaUploadDir);
        const memoryStatus = getMemoryHandoffStatus(settings.skillRoot);

        sendJson(res, 200, {
          settings,
          appRoot: ROOT,
          lanUrls: getLanUrls(activePort),
          ...providersInfo,
          skillsInfo,
          videos,
          uploadedMedia,
          tunnels: tunnelsOverview,
          memoryStatus,
          jobs: runner.listJobs(),
        });
        return;
      }

      if (pathname === "/api/settings" && (req.method === "POST" || req.method === "PUT")) {
        const patch = await readBody(req);
        const settings = saveSettings(patch);
        const extraSkillRoots = [settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])];
        const [providersInfo, skillsInfo] = await Promise.all([
          getProvidersCatalog({ force: true, settings }),
          Promise.resolve(discoverSkills(settings.skillRoot, extraSkillRoots)),
        ]);
        sendJson(res, 200, {
          ok: true,
          settings,
          ...providersInfo,
          skillsInfo,
          videos: listVideoProjects(settings.skillRoot, [settings.videoOutputDir]),
          uploadedMedia: listUploadedMedia(settings.mediaUploadDir),
          memoryStatus: getMemoryHandoffStatus(settings.skillRoot),
        });
        return;
      }

      // ================= SKILL IMPORT (FOLDER / .ZIP / .RAR / CUSTOM SAVE DIR) =================
      if (pathname === "/api/pick-path" && req.method === "POST") {
        const body = await readBody(req);
        const result = await pickNativePath({ kind: body.kind || "folder", title: body.title || "Chọn thư mục" });
        sendJson(res, 200, result);
        return;
      }

      if (pathname === "/api/skills/import-archive" && req.method === "POST") {
        const settings = loadSettings();
        const rawName = decodeURIComponent(String(req.headers["x-file-name"] || "skill.zip"));
        const targetDir = decodeURIComponent(String(req.headers["x-target-dir"] || "")).trim() || settings.customSkillSaveDir || settings.skillRoot;
        const overwrite = String(req.headers["x-overwrite"] ?? "1") !== "0";
        const syncLinks = String(req.headers["x-sync-links"] ?? (settings.syncSkillLinks ? "1" : "0")) !== "0";
        const customSkillName = decodeURIComponent(String(req.headers["x-skill-name"] || "")).trim();

        const ext = path.extname(rawName) || ".zip";
        const { tmpFile } = await streamRequestToTempFile(req, ext);
        try {
          const result = await importSkillFromArchiveFile({
            archivePath: tmpFile,
            originalName: rawName,
            targetRoot: targetDir,
            overwrite,
            syncLinks,
            customSkillName,
          });
          const nextSettings = saveSettings({ customSkillSaveDir: targetDir });
          const skillsInfo = discoverSkills(nextSettings.skillRoot, [nextSettings.customSkillSaveDir, ...(nextSettings.extraSkillRoots || [])]);
          sendJson(res, 200, { ...result, settings: nextSettings, skillsInfo });
        } finally {
          try { fs.unlinkSync(tmpFile); } catch {}
        }
        return;
      }

      if (pathname === "/api/skills/import-folder-upload" && req.method === "POST") {
        const body = await readBody(req, 80_000_000);
        const settings = loadSettings();
        const targetDir = String(body.targetDir || "").trim() || settings.customSkillSaveDir || settings.skillRoot;
        const result = await importSkillFromUploadedFiles({
          files: body.files || [],
          folderName: body.folderName || "imported-skill",
          targetRoot: targetDir,
          overwrite: body.overwrite !== false,
          syncLinks: body.syncLinks ?? settings.syncSkillLinks,
          customSkillName: body.customSkillName || "",
        });
        const nextSettings = saveSettings({ customSkillSaveDir: targetDir });
        const skillsInfo = discoverSkills(nextSettings.skillRoot, [nextSettings.customSkillSaveDir, ...(nextSettings.extraSkillRoots || [])]);
        sendJson(res, 200, { ...result, settings: nextSettings, skillsInfo });
        return;
      }

      if (pathname === "/api/skills/import-local" && req.method === "POST") {
        const body = await readBody(req);
        const settings = loadSettings();
        const targetDir = String(body.targetDir || "").trim() || settings.customSkillSaveDir || settings.skillRoot;
        const result = await importSkillFromLocalPath({
          sourcePath: String(body.sourcePath || "").trim(),
          targetRoot: targetDir,
          overwrite: body.overwrite !== false,
          syncLinks: body.syncLinks ?? settings.syncSkillLinks,
          customSkillName: body.customSkillName || "",
        });
        const nextSettings = saveSettings({ customSkillSaveDir: targetDir });
        const skillsInfo = discoverSkills(nextSettings.skillRoot, [nextSettings.customSkillSaveDir, ...(nextSettings.extraSkillRoots || [])]);
        sendJson(res, 200, { ...result, settings: nextSettings, skillsInfo });
        return;
      }

      const delSkillMatch = pathname.match(/^\/api\/skills\/([^/]+)$/);
      if (delSkillMatch && req.method === "DELETE") {
        const id = decodeURIComponent(delSkillMatch[1]);
        const settings = loadSettings();
        const roots = [settings.skillRoot, settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])];
        const result = deleteCustomSkill(id, roots);
        const skillsInfo = discoverSkills(settings.skillRoot, [settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])]);
        sendJson(res, result.ok ? 200 : 400, { ...result, skillsInfo });
        return;
      }

      // ================= MOBILE APP PHOTOS / MEDIA UPLOAD (iPad · iPhone · Android) =================
      if (pathname === "/api/media-upload" && req.method === "POST") {
        const settings = loadSettings();
        const fileName = decodeURIComponent(String(req.headers["x-file-name"] || "photo.jpg"));
        const customDir = decodeURIComponent(String(req.headers["x-target-dir"] || "")).trim();
        const uploadDir = customDir || settings.mediaUploadDir;
        const saved = await saveUploadedMediaStream(req, { uploadDir, fileName });
        sendJson(res, 200, { ok: true, file: saved, uploadedMedia: listUploadedMedia(settings.mediaUploadDir) });
        return;
      }

      if (pathname === "/api/media-uploads" && req.method === "GET") {
        const settings = loadSettings();
        sendJson(res, 200, { ok: true, uploadedMedia: listUploadedMedia(settings.mediaUploadDir) });
        return;
      }

      if (pathname === "/api/media-uploads" && req.method === "DELETE") {
        const body = await readBody(req);
        const settings = loadSettings();
        const out = deleteUploadedMedia(settings.mediaUploadDir, body.path);
        sendJson(res, out.ok ? 200 : 400, { ...out, uploadedMedia: listUploadedMedia(settings.mediaUploadDir) });
        return;
      }

      // ================= PREVIEW & MULTI-SESSION / MULTI-AI JOBS =================
      if (pathname === "/api/preview-command" && req.method === "POST") {
        const body = await readBody(req);
        const settings = loadSettings();
        const sharedMemoryHint = settings.autoInjectSharedMemory ? buildSharedMemoryPromptHint(settings.skillRoot) : "";
        const prompt = body.rawPrompt?.trim()
          ? body.rawPrompt.trim()
          : composeSkillPrompt({
              skillId: body.skillId || settings.selectedSkill,
              providerId: body.providerId || settings.selectedProvider,
              brief: body.brief || "",
              topic: body.topic || "",
              format: body.format || "9:16",
              character: body.character || "",
              stickMode: body.stickMode || "",
              renderStage: body.renderStage || "draft",
              projectFolder: body.projectFolder || "",
              outputDir: body.outputDir ?? settings.videoOutputDir,
              mediaFiles: body.mediaFiles || [],
              sharedMemoryHint,
            });
        const providersInfo = await getProvidersCatalog({ settings });
        const agyModels = providersInfo.providers.find((p) => p.id === "antigravity")?.models;
        const preview = buildInvocationPreview({
          providerId: body.providerId || "claude",
          model: body.model || "",
          effort: body.effort || "",
          prompt,
          skillRoot: settings.skillRoot,
          prefs: settings.providerPrefs?.[body.providerId] || {},
          agyModels,
        });
        sendJson(res, 200, { ok: true, prompt, display: preview.display });
        return;
      }

      if (pathname === "/api/jobs" && req.method === "GET") {
        sendJson(res, 200, { jobs: runner.listJobs() });
        return;
      }

      if (pathname === "/api/jobs" && req.method === "POST") {
        const body = await readBody(req);
        const settings = saveSettings({
          selectedSkill: body.skillId || undefined,
          selectedProvider: body.providerId || undefined,
          ...(body.outputDir ? { videoOutputDir: body.outputDir } : {}),
          providerPrefs: body.providerId
            ? {
                [body.providerId]: {
                  model: body.model ?? "",
                  effort: body.effort ?? "",
                },
              }
            : undefined,
        });

        const providersInfo = await getProvidersCatalog({ settings });
        const agyModels = providersInfo.providers.find((p) => p.id === "antigravity")?.models;
        const extraRoots = [settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])];
        const skillsInfo = discoverSkills(settings.skillRoot, extraRoots);
        const skillObj = skillsInfo.skills.find((s) => s.id === body.skillId);
        const sharedMemoryHint = settings.autoInjectSharedMemory ? buildSharedMemoryPromptHint(settings.skillRoot) : "";
        const effOutputDir = body.outputDir ?? settings.videoOutputDir;

        const prompt = body.rawPrompt?.trim()
          ? body.rawPrompt.trim()
          : composeSkillPrompt({
              skillId: body.skillId,
              providerId: body.providerId,
              brief: body.brief || "",
              topic: body.topic || "",
              format: body.format || "9:16",
              character: body.character || "",
              stickMode: body.stickMode || "",
              renderStage: body.renderStage || "draft",
              projectFolder: body.projectFolder || "",
              outputDir: effOutputDir,
              mediaFiles: body.mediaFiles || [],
              sharedMemoryHint,
            });

        const result = runner.startJob({
          skillId: body.skillId,
          skillTitle: skillObj?.title || body.skillId,
          providerId: body.providerId,
          model: body.model || "",
          effort: body.effort || "",
          prompt,
          skillRoot: settings.skillRoot,
          extraSkillDirs: extraRoots,
          projectFolder: body.projectFolder || "",
          outputDir: effOutputDir,
          sessionLabel: body.sessionLabel || "",
          prefs: settings.providerPrefs?.[body.providerId] || {},
          agyModels,
          resumeSessionId: body.resumeSessionId || "",
        });

        sendJson(res, result.ok ? 200 : 400, result);
        return;
      }

      // Chạy đồng thời NHIỀU PHIÊN trên CÙNG 1 AI hoặc NHIỀU PHIÊN trên NHIỀU AI (Claude · Codex · Antigravity)
      if (pathname === "/api/jobs/batch" && req.method === "POST") {
        const body = await readBody(req);
        const items = Array.isArray(body.sessions) ? body.sessions : [];
        if (!items.length) {
          sendJson(res, 400, { ok: false, error: "Danh sách phiên chạy đồng thời đang trống." });
          return;
        }
        const settings = loadSettings();
        if (body.outputDir) saveSettings({ videoOutputDir: body.outputDir });
        const providersInfo = await getProvidersCatalog({ settings });
        const agyModels = providersInfo.providers.find((p) => p.id === "antigravity")?.models;
        const extraRoots = [settings.customSkillSaveDir, ...(settings.extraSkillRoots || [])];
        const skillsInfo = discoverSkills(settings.skillRoot, extraRoots);
        const sharedMemoryHint = settings.autoInjectSharedMemory ? buildSharedMemoryPromptHint(settings.skillRoot) : "";
        const batchId = `batch_${Date.now().toString(36)}`;
        const usedFolders = new Set();

        const started = [];
        const errors = [];

        items.forEach((s, idx) => {
          const provId = s.providerId || settings.selectedProvider || "claude";
          const skId = s.skillId || settings.selectedSkill || "stickman-video";
          const skillObj = skillsInfo.skills.find((x) => x.id === skId);
          const effOutputDir = s.outputDir ?? body.outputDir ?? settings.videoOutputDir;

          // Đảm bảo mỗi phiên song song có thư mục dự án riêng biệt để không xung đột khi render đồng thời
          let folder = String(s.projectFolder || "").trim();
          if (!folder) {
            const d = new Date().toISOString().slice(0, 10);
            folder = `_video/${d}-${skId.replace(/-video$/, "")}-${provId}-s${idx + 1}`;
          } else if (usedFolders.has(folder.toLowerCase()) || items.length > 1) {
            const hasSuffix = new RegExp(`-${provId}-s\\d+$`, "i").test(folder);
            if (!hasSuffix && usedFolders.has(folder.toLowerCase())) {
              folder = `${folder}-${provId}-s${idx + 1}`;
            } else if (!hasSuffix && items.filter((it) => (it.projectFolder || "") === s.projectFolder).length > 1) {
              folder = `${folder}-${provId}-s${idx + 1}`;
            }
          }
          usedFolders.add(folder.toLowerCase());

          const prompt = s.rawPrompt?.trim()
            ? s.rawPrompt.trim()
            : composeSkillPrompt({
                skillId: skId,
                providerId: provId,
                brief: s.brief || "",
                topic: s.topic || "thoi-su",
                format: s.format || "9:16",
                character: s.character || "",
                stickMode: s.stickMode || "",
                renderStage: s.renderStage || "draft",
                projectFolder: folder,
                outputDir: effOutputDir,
                mediaFiles: s.mediaFiles || body.mediaFiles || [],
                sharedMemoryHint,
              });

          const r = runner.startJob({
            batchId,
            sessionLabel: s.sessionLabel || `#${idx + 1} ${provId.toUpperCase()}`,
            skillId: skId,
            skillTitle: skillObj?.title || skId,
            providerId: provId,
            model: s.model || settings.providerPrefs?.[provId]?.model || "",
            effort: s.effort ?? settings.providerPrefs?.[provId]?.effort ?? "",
            prompt,
            skillRoot: settings.skillRoot,
            extraSkillDirs: extraRoots,
            projectFolder: folder,
            outputDir: effOutputDir,
            prefs: settings.providerPrefs?.[provId] || {},
            agyModels,
          });

          if (r.ok) started.push(r.job);
          else errors.push({ index: idx + 1, providerId: provId, error: r.error });
        });

        sendJson(res, started.length > 0 ? 200 : 400, {
          ok: started.length > 0,
          batchId,
          started,
          errors,
          error: started.length === 0 ? errors.map((e) => `[${e.providerId}] ${e.error}`).join(" | ") : "",
        });
        return;
      }

      if (pathname === "/api/jobs/stop-all" && req.method === "POST") {
        sendJson(res, 200, runner.stopAllJobs());
        return;
      }

      if (pathname === "/api/jobs/clear-finished" && req.method === "POST") {
        sendJson(res, 200, runner.clearFinishedJobs());
        return;
      }

      const stopMatch = pathname.match(/^\/api\/jobs\/([^/]+)\/stop$/);
      if (stopMatch && req.method === "POST") {
        const result = runner.stopJob(decodeURIComponent(stopMatch[1]));
        sendJson(res, result.ok ? 200 : 400, result);
        return;
      }

      // ================= CLOUDFLARE TUNNEL CRUD =================
      if (pathname === "/api/tunnels" && req.method === "GET") {
        sendJson(res, 200, { ok: true, ...(await tunnelManager.getOverview()) });
        return;
      }

      if (pathname === "/api/tunnels" && req.method === "POST") {
        const body = await readBody(req);
        const result = tunnelManager.createTunnel(body);
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      if (pathname === "/api/tunnels/quick-web" && req.method === "POST") {
        const result = tunnelManager.ensureQuickWebTunnel();
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      if (pathname === "/api/tunnels/adopt-external" && req.method === "POST") {
        const body = await readBody(req);
        const result = await tunnelManager.adoptExternalProcess(body.pid, body.name || "");
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      if (pathname === "/api/tunnels/stop-external" && req.method === "POST") {
        const body = await readBody(req);
        const result = tunnelManager.stopExternalProcess(body.pid);
        sendJson(res, 200, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      const tunActionMatch = pathname.match(/^\/api\/tunnels\/([^/]+)\/(start|stop|restart)$/);
      if (tunActionMatch && req.method === "POST") {
        const id = decodeURIComponent(tunActionMatch[1]);
        const act = tunActionMatch[2];
        let result;
        if (act === "start") result = tunnelManager.startTunnel(id);
        else if (act === "stop") result = tunnelManager.stopTunnel(id);
        else {
          tunnelManager.stopTunnel(id);
          result = tunnelManager.startTunnel(id);
        }
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      const tunIdMatch = pathname.match(/^\/api\/tunnels\/([^/]+)$/);
      if (tunIdMatch && req.method === "PUT") {
        const id = decodeURIComponent(tunIdMatch[1]);
        const body = await readBody(req);
        const result = tunnelManager.updateTunnel(id, body);
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }
      if (tunIdMatch && req.method === "DELETE") {
        const id = decodeURIComponent(tunIdMatch[1]);
        const result = tunnelManager.deleteTunnel(id);
        sendJson(res, result.ok ? 200 : 400, { ...result, overview: await tunnelManager.getOverview() });
        return;
      }

      // ================= SHARED MEMORY & HANDOFF (ANTIGRAVITY · CLAUDE · CODEX) =================
      if (pathname === "/api/memory/status" && req.method === "GET") {
        const settings = loadSettings();
        const ws = urlObj.searchParams.get("workspace") || settings.skillRoot;
        sendJson(res, 200, { ok: true, status: getMemoryHandoffStatus(ws), appStatus: getMemoryHandoffStatus(ROOT) });
        return;
      }

      if (pathname === "/api/memory/init" && req.method === "POST") {
        const body = await readBody(req);
        const settings = loadSettings();
        const ws = body.workspace || settings.skillRoot;
        const result = initOrSyncMemoryHandoff(ws, { overwrite: Boolean(body.overwrite) });
        sendJson(res, 200, result);
        return;
      }

      if (pathname === "/api/memory/doc" && req.method === "GET") {
        const settings = loadSettings();
        const ws = urlObj.searchParams.get("workspace") || settings.skillRoot;
        const file = urlObj.searchParams.get("file") || "MEMORY.md";
        sendJson(res, 200, readMemoryDoc(ws, file));
        return;
      }

      if (pathname === "/api/memory/doc" && (req.method === "PUT" || req.method === "POST")) {
        const body = await readBody(req);
        const settings = loadSettings();
        const ws = body.workspace || settings.skillRoot;
        const out = writeMemoryDoc(ws, body.file || "MEMORY.md", body.content || "");
        sendJson(res, 200, { ...out, status: getMemoryHandoffStatus(ws) });
        return;
      }

      if (pathname === "/api/memory/append-note" && req.method === "POST") {
        const body = await readBody(req);
        const settings = loadSettings();
        const ws = body.workspace || settings.skillRoot;
        const out = appendMemoryEntry(ws, { agent: body.agent || "User", note: body.note || "" });
        sendJson(res, 200, { ...out, status: getMemoryHandoffStatus(ws) });
        return;
      }

      if (pathname === "/api/memory/record-job" && req.method === "POST") {
        const body = await readBody(req);
        const settings = loadSettings();
        const job = runner.getJob(body.jobId);
        if (!job) {
          sendJson(res, 404, { ok: false, error: "Không tìm thấy phiên chạy." });
          return;
        }
        const ws = body.workspace || settings.skillRoot;
        const out = appendHandoffSession(ws, job);
        sendJson(res, 200, { ...out, status: getMemoryHandoffStatus(ws) });
        return;
      }

      // ================= VIDEOS & MEDIA STREAMING =================
      if (pathname === "/api/videos" && req.method === "GET") {
        const settings = loadSettings();
        sendJson(res, 200, { videos: listVideoProjects(settings.skillRoot, [settings.videoOutputDir]) });
        return;
      }

      if (pathname === "/api/doc-content" && req.method === "GET") {
        const settings = loadSettings();
        const allowed = getAllowedRoots(settings, [ROOT]);
        const target = urlObj.searchParams.get("path") || "";
        if (!isAllowedMediaPath(target, allowed) || !fs.existsSync(target)) {
          sendJson(res, 404, { ok: false, error: "Không tìm thấy tài liệu." });
          return;
        }
        sendJson(res, 200, { ok: true, content: fs.readFileSync(target, "utf8").slice(0, 160_000) });
        return;
      }

      if (pathname === "/api/media" && req.method === "GET") {
        const settings = loadSettings();
        const allowed = getAllowedRoots(settings, [ROOT]);
        const target = urlObj.searchParams.get("path") || "";
        const forceDownload = urlObj.searchParams.get("download") === "1";
        if (!isAllowedMediaPath(target, allowed)) {
          res.writeHead(403);
          res.end("Forbidden");
          return;
        }
        serveMediaWithRange(req, res, target, forceDownload);
        return;
      }

      if (pathname === "/api/open-path" && req.method === "POST") {
        const { path: targetPath } = await readBody(req);
        const settings = loadSettings();
        const allowed = getAllowedRoots(settings, [ROOT]);
        if (!targetPath || !isAllowedMediaPath(targetPath, allowed)) {
          sendJson(res, 403, { ok: false, error: "Đường dẫn ngoài vùng cho phép." });
          return;
        }
        const ok = openInOsExplorer(targetPath);
        sendJson(res, ok ? 200 : 400, { ok });
        return;
      }

      if (pathname === "/api/launch-antigravity" && req.method === "POST") {
        const settings = loadSettings();
        const appPath = resolveAntigravityDesktopApp();
        if (!appPath) {
          sendJson(res, 404, { ok: false, error: "Không tìm thấy ứng dụng Antigravity Desktop trên máy." });
          return;
        }
        if (process.platform === "darwin") {
          spawn("open", ["-a", appPath, settings.skillRoot], { detached: true }).unref();
        } else {
          spawn(appPath, [settings.skillRoot], { detached: true, windowsHide: true }).unref();
        }
        sendJson(res, 200, { ok: true, appPath });
        return;
      }

      // Phục vụ giao diện Web tĩnh từ dist/ (khi chạy production, qua Cloudflare Tunnel, LAN hoặc bên trong Tauri Desktop)
      if (fs.existsSync(DIST_DIR)) {
        const relPath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
        const fullStatic = path.resolve(DIST_DIR, relPath);
        if (fullStatic.startsWith(path.resolve(DIST_DIR)) && fs.existsSync(fullStatic) && fs.statSync(fullStatic).isFile()) {
          const ext = path.extname(fullStatic).toLowerCase();
          res.writeHead(200, { "Content-Type": MIME[ext] || "application/octet-stream" });
          fs.createReadStream(fullStatic).pipe(res);
          return;
        }
        const indexHtml = path.join(DIST_DIR, "index.html");
        if (fs.existsSync(indexHtml)) {
          res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
          fs.createReadStream(indexHtml).pipe(res);
          return;
        }
      }

      sendJson(res, 404, { ok: false, error: "Chưa build frontend (chạy `npm run build` hoặc `npm run dev`)." });
    } catch (err) {
      sendJson(res, 500, { ok: false, error: err?.message || String(err) });
    }
  });

  const heartbeat = setInterval(() => {
    for (const c of sseClients) {
      try {
        c.write(": ping\n\n");
      } catch {
        sseClients.delete(c);
      }
    }
  }, 20_000);
  heartbeat.unref?.();

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });

  const addr = server.address();
  activePort = typeof addr === "object" && addr ? addr.port : port;

  tunnelManager = createTunnelManager({
    defaultPort: activePort,
    onEvent: (ev) => broadcast(ev),
  });

  return {
    server,
    port: activePort,
    tunnelManager,
    url: `http://${host === "0.0.0.0" ? "localhost" : host}:${activePort}`,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const port = Number(process.env.DOTO_SKILL_PORT || 4320);
  const host = process.argv.includes("--lan") ? "0.0.0.0" : (process.env.DOTO_SKILL_HOST || "127.0.0.1");
  startServer({ port, host }).then(async ({ url, tunnelManager }) => {
    console.log(`✓ Doto Skill Studio Server v0.2.0 đang lắng nghe tại ${url}`);
    if (process.argv.includes("--tunnel")) {
      const r = tunnelManager.ensureQuickWebTunnel();
      if (r.ok) console.log("▸ Đang khởi tạo Cloudflare Quick Tunnel cho Web...");
    }
  });
}

// Quét các dự án video đã tạo trong _video/, _demo/ và các thư mục lưu tuỳ chỉnh (videoOutputDir) để xem/tải về thiết bị.
import fs from "node:fs";
import path from "node:path";

function findRenderFiles(projectDir) {
  const rendersDir = path.join(projectDir, "renders");
  const mp4s = [];
  const sheets = [];
  const scanDirs = [rendersDir, projectDir];
  const seen = new Set();

  for (const dir of scanDirs) {
    if (!fs.existsSync(dir)) continue;
    let files = [];
    try {
      files = fs.readdirSync(dir);
    } catch {
      continue;
    }
    for (const f of files) {
      const full = path.join(dir, f);
      if (seen.has(full)) continue;
      let st;
      try {
        st = fs.statSync(full);
      } catch {
        continue;
      }
      if (!st.isFile()) continue;
      seen.add(full);
      if (/\.mp4$/i.test(f)) {
        mp4s.push({
          name: f,
          path: full,
          size: st.size,
          mtimeMs: st.mtimeMs,
          priority: /^final/i.test(f) ? 3 : /xem-nhanh/i.test(f) ? 2 : 1,
        });
      } else if (dir === rendersDir && (/\.sheet\.(jpg|jpeg|png)$/i.test(f) || /\.(jpg|png)$/i.test(f))) {
        sheets.push({ name: f, path: full, mtimeMs: st.mtimeMs });
      }
    }
  }
  mp4s.sort((a, b) => b.priority - a.priority || b.mtimeMs - a.mtimeMs);
  sheets.sort((a, b) => b.mtimeMs - a.mtimeMs);
  return { mp4s, sheets };
}

export function listVideoProjects(skillRoot, extraVideoDirs = []) {
  const buckets = [];
  const seenBuckets = new Set();

  const pushBucket = (dir, kind) => {
    if (!dir || !String(dir).trim()) return;
    const resolved = path.resolve(String(dir).trim());
    const norm = process.platform === "win32" ? resolved.toLowerCase() : resolved;
    if (seenBuckets.has(norm) || !fs.existsSync(resolved)) return;
    seenBuckets.add(norm);
    buckets.push({ dir: resolved, kind });
  };

  if (skillRoot && fs.existsSync(skillRoot)) {
    pushBucket(path.join(skillRoot, "_video"), "video");
    pushBucket(path.join(skillRoot, "_demo"), "demo");
  }
  for (const d of Array.isArray(extraVideoDirs) ? extraVideoDirs : [extraVideoDirs]) {
    if (d) pushBucket(d, "video");
  }

  const items = [];
  const seenProjects = new Set();

  for (const b of buckets) {
    let entries = [];
    try {
      entries = fs.readdirSync(b.dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const e of entries) {
      if (!e.isDirectory() || e.name.startsWith("_") || e.name.startsWith(".")) continue;
      const pDir = path.join(b.dir, e.name);
      const normP = process.platform === "win32" ? pDir.toLowerCase() : pDir;
      if (seenProjects.has(normP)) continue;
      seenProjects.add(normP);

      const { mp4s, sheets } = findRenderFiles(pDir);
      let title = e.name;
      let topic = "";
      let format = "";
      let duration = 0;

      const sbFile = path.join(pDir, "storyboard.json");
      const tlFile = path.join(pDir, "public", "timeline.json");
      try {
        if (fs.existsSync(tlFile)) {
          const tl = JSON.parse(fs.readFileSync(tlFile, "utf8"));
          title = tl.meta?.title || title;
          topic = tl.meta?.topic || "";
          format = tl.meta?.format || (tl.meta?.width && tl.meta?.height ? `${tl.meta.width}×${tl.meta.height}` : "");
          duration = Number(tl.meta?.duration || 0);
        } else if (fs.existsSync(sbFile)) {
          const sb = JSON.parse(fs.readFileSync(sbFile, "utf8"));
          title = sb.meta?.title || title;
          topic = sb.meta?.topic || "";
          format = sb.meta?.format || "";
        }
      } catch {
        // ignore
      }

      const factsFile = path.join(pDir, "FACTS.md");
      const reviewFile = path.join(pDir, "REVIEW.md");
      let mtimeMs = 0;
      try {
        mtimeMs = Math.max(fs.statSync(pDir).mtimeMs, mp4s[0]?.mtimeMs || 0);
      } catch {
        mtimeMs = Date.now();
      }

      items.push({
        id: `${b.kind}:${e.name}:${items.length}`,
        slug: e.name,
        kind: b.kind,
        title,
        topic,
        format: format || (fs.existsSync(path.join(pDir, "index.html")) && !fs.existsSync(sbFile) ? "16:9 (3D)" : "9:16"),
        duration,
        projectDir: pDir,
        mtimeMs,
        primaryVideo: mp4s[0] || null,
        videos: mp4s,
        sheet: sheets[0] || null,
        hasFacts: fs.existsSync(factsFile),
        hasReview: fs.existsSync(reviewFile),
      });
    }
  }

  items.sort((a, b) => b.mtimeMs - a.mtimeMs);
  return items;
}

/** Kiểm tra đường dẫn media có nằm trong danh sách các thư mục được phép truy cập hay không */
export function isAllowedMediaPath(targetPath, allowedRootOrList) {
  if (!targetPath || !allowedRootOrList) return false;
  const roots = Array.isArray(allowedRootOrList) ? allowedRootOrList : [allowedRootOrList];
  const resolvedTarget = path.resolve(targetPath);
  const normT = process.platform === "win32" ? resolvedTarget.toLowerCase() : resolvedTarget;

  for (const r of roots) {
    if (!r) continue;
    const resolvedRoot = path.resolve(r);
    const normR = process.platform === "win32" ? resolvedRoot.toLowerCase() : resolvedRoot;
    if (normT === normR || normT.startsWith(normR + path.sep)) {
      return true;
    }
  }
  return false;
}

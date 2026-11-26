// Xử lý tải ảnh & video trực tiếp từ App Photos trên iPhone / iPad (iOS/iPadOS), Android Photos/Gallery và PC.
// Truyền luồng nhị phân (binary stream) trực tiếp ra ổ đĩa giúp tải mượt cả video 4K dung lượng lớn từ điện thoại.
import fs from "node:fs";
import path from "node:path";

function safeMediaFilename(rawName) {
  const base = path.basename(String(rawName || "photo.jpg"));
  const ext = path.extname(base).toLowerCase() || ".bin";
  const stem = path
    .basename(base, path.extname(base))
    .replace(/[^a-zA-Z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60) || "media";
  const stamp = Date.now().toString(36);
  return `${stem}-${stamp}${ext}`;
}

export function saveUploadedMediaStream(req, { uploadDir, fileName, maxBytes = 1024 * 1024 * 1024 }) {
  return new Promise((resolve, reject) => {
    const today = new Date().toISOString().slice(0, 10);
    const destFolder = path.resolve(uploadDir, today);
    fs.mkdirSync(destFolder, { recursive: true });
    const safeName = safeMediaFilename(fileName);
    const fullPath = path.join(destFolder, safeName);
    const ws = fs.createWriteStream(fullPath);
    let bytes = 0;

    req.on("data", (chunk) => {
      bytes += chunk.length;
      if (bytes > maxBytes) {
        ws.destroy();
        try { fs.unlinkSync(fullPath); } catch {}
        reject(new Error("Dung lượng tệp vượt quá giới hạn 1GB."));
        req.destroy();
      }
    });

    req.pipe(ws);

    ws.on("finish", () => {
      const ext = path.extname(safeName).toLowerCase();
      const isVideo = /\.(mp4|mov|m4v|webm|mkv|avi)$/i.test(ext);
      const isImage = /\.(jpg|jpeg|png|webp|heic|heif|gif)$/i.test(ext);
      resolve({
        id: `${today}/${safeName}`,
        name: safeName,
        originalName: fileName || safeName,
        path: fullPath,
        size: bytes,
        kind: isVideo ? "video" : isImage ? "image" : "file",
        createdAt: Date.now(),
      });
    });

    ws.on("error", (err) => {
      try { fs.unlinkSync(fullPath); } catch {}
      reject(err);
    });
    req.on("error", (err) => {
      ws.destroy();
      try { fs.unlinkSync(fullPath); } catch {}
      reject(err);
    });
  });
}

export function listUploadedMedia(uploadDir) {
  if (!uploadDir || !fs.existsSync(uploadDir)) return [];
  const items = [];
  try {
    const subdirs = fs.readdirSync(uploadDir, { withFileTypes: true }).filter((d) => d.isDirectory());
    for (const d of subdirs) {
      const subPath = path.join(uploadDir, d.name);
      for (const f of fs.readdirSync(subPath)) {
        const full = path.join(subPath, f);
        let st;
        try {
          st = fs.statSync(full);
          if (!st.isFile()) continue;
        } catch {
          continue;
        }
        const ext = path.extname(f).toLowerCase();
        const isVideo = /\.(mp4|mov|m4v|webm|mkv|avi)$/i.test(ext);
        const isImage = /\.(jpg|jpeg|png|webp|heic|heif|gif)$/i.test(ext);
        items.push({
          id: `${d.name}/${f}`,
          name: f,
          originalName: f,
          path: full,
          size: st.size,
          kind: isVideo ? "video" : isImage ? "image" : "file",
          createdAt: st.mtimeMs,
        });
      }
    }
  } catch {
    // ignore
  }
  items.sort((a, b) => b.createdAt - a.createdAt);
  return items.slice(0, 80);
}

export function deleteUploadedMedia(uploadDir, targetPath) {
  if (!uploadDir || !targetPath) return { ok: false, error: "Thiếu đường dẫn tệp." };
  const resolvedRoot = path.resolve(uploadDir);
  const resolvedTarget = path.resolve(targetPath);
  const normRoot = process.platform === "win32" ? resolvedRoot.toLowerCase() + path.sep : resolvedRoot + path.sep;
  const normTarget = process.platform === "win32" ? resolvedTarget.toLowerCase() : resolvedTarget;
  if (!normTarget.startsWith(normRoot) || !fs.existsSync(resolvedTarget)) {
    return { ok: false, error: "Tệp không hợp lệ hoặc không tồn tại." };
  }
  fs.unlinkSync(resolvedTarget);
  return { ok: true };
}

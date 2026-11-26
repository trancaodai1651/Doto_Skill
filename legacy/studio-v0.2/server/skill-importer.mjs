// Bộ nhập (Import) Skill từ Thư mục, File nén (.zip, .rar, .7z, .tar.gz) hoặc kéo-thả trực tiếp trên Web/Desktop.
// Hỗ trợ tuỳ chỉnh nơi lưu (customSkillSaveDir) và tự động tạo liên kết (junction/symlink) cho Claude, Codex & Antigravity.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolveUnrarBinary, standaloneEnvironment } from "./cli-resolver.mjs";
import { CORE_SKILL_IDS } from "./skills.mjs";

const execFileAsync = promisify(execFile);
const HOME = os.homedir();
const IS_WIN = process.platform === "win32";

function sanitizeSlug(name) {
  const cleaned = String(name || "custom-skill")
    .trim()
    .replace(/\.(zip|rar|7z|tar\.gz|tgz|tar)$/i, "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return cleaned || `skill-${Date.now().toString(36)}`;
}

/** Trình giải nén ZIP thuần Node.js (dùng zlib.inflateRawSync) làm phương án dự phòng không phụ thuộc phần mềm ngoài */
function extractZipPureNode(zipPath, destDir) {
  const buf = fs.readFileSync(zipPath);
  // Tìm End of Central Directory (0x06054b50)
  let eocdOffset = -1;
  const minPos = Math.max(0, buf.length - 65557);
  for (let i = buf.length - 22; i >= minPos; i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocdOffset = i;
      break;
    }
  }
  if (eocdOffset < 0) {
    throw new Error("Tệp ZIP không hợp lệ (không tìm thấy Central Directory).");
  }
  const totalEntries = buf.readUInt16LE(eocdOffset + 10);
  const cdOffset = buf.readUInt32LE(eocdOffset + 16);
  let ptr = cdOffset;
  const resolvedDest = path.resolve(destDir);

  for (let idx = 0; idx < totalEntries; idx++) {
    if (ptr + 46 > buf.length || buf.readUInt32LE(ptr) !== 0x02014b50) break;
    const method = buf.readUInt16LE(ptr + 10);
    const compSize = buf.readUInt32LE(ptr + 20);
    const nameLen = buf.readUInt16LE(ptr + 28);
    const extraLen = buf.readUInt16LE(ptr + 30);
    const commentLen = buf.readUInt16LE(ptr + 32);
    const localHeaderOffset = buf.readUInt32LE(ptr + 42);
    const rawName = buf.subarray(ptr + 46, ptr + 46 + nameLen).toString("utf8");
    ptr += 46 + nameLen + extraLen + commentLen;

    const normRel = rawName.replace(/\\/g, "/");
    if (!normRel || normRel.startsWith("__MACOSX/") || normRel.endsWith(".DS_Store")) continue;

    const outPath = path.resolve(resolvedDest, normRel);
    const safePrefix = IS_WIN ? resolvedDest.toLowerCase() + path.sep : resolvedDest + path.sep;
    const checkTarget = IS_WIN ? outPath.toLowerCase() : outPath;
    if (checkTarget !== (IS_WIN ? resolvedDest.toLowerCase() : resolvedDest) && !checkTarget.startsWith(safePrefix)) {
      throw new Error(`Phát hiện đường dẫn không an toàn trong file nén: ${rawName}`);
    }

    if (normRel.endsWith("/")) {
      fs.mkdirSync(outPath, { recursive: true });
      continue;
    }

    if (buf.readUInt32LE(localHeaderOffset) !== 0x04034b50) {
      throw new Error(`Local header lỗi cho tệp: ${rawName}`);
    }
    const lfNameLen = buf.readUInt16LE(localHeaderOffset + 26);
    const lfExtraLen = buf.readUInt16LE(localHeaderOffset + 28);
    const dataStart = localHeaderOffset + 30 + lfNameLen + lfExtraLen;
    const compData = buf.subarray(dataStart, dataStart + compSize);

    let fileBytes;
    if (method === 0) {
      fileBytes = compData;
    } else if (method === 8) {
      fileBytes = zlib.inflateRawSync(compData);
    } else {
      throw new Error(`Phương thức nén ZIP (${method}) cần bsdtar hoặc 7z.`);
    }

    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, fileBytes);
  }
}

/** Giải nén đa định dạng (.zip, .rar, .7z, .tar.gz) an toàn tuyệt đối */
export async function extractArchiveFile(archivePath, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  const lower = archivePath.toLowerCase();
  const head = Buffer.alloc(8);
  const fd = fs.openSync(archivePath, "r");
  try {
    fs.readSync(fd, head, 0, 8, 0);
  } finally {
    fs.closeSync(fd);
  }
  const isRar = lower.endsWith(".rar") || (head[0] === 0x52 && head[1] === 0x61 && head[2] === 0x72 && head[3] === 0x21);
  const isZip = lower.endsWith(".zip") || (head[0] === 0x50 && head[1] === 0x4b);

  const errors = [];

  // 1) Với RAR: Ưu tiên UnRAR / WinRAR / unar / 7z nếu có, sau đó dùng bsdtar (tar hệ thống có libarchive hỗ trợ RAR4/RAR5)
  if (isRar) {
    const unrar = resolveUnrarBinary();
    if (unrar) {
      try {
        if (unrar.tool === "unrar" || unrar.tool === "winrar") {
          const destWithSlash = destDir.endsWith(path.sep) ? destDir : `${destDir}${path.sep}`;
          await execFileAsync(unrar.file, [...unrar.prefixArgs, "x", "-o+", "-y", archivePath, destWithSlash], {
            timeout: 90_000,
            windowsHide: true,
            env: standaloneEnvironment(),
          });
          return { ok: true, method: unrar.tool };
        }
        if (unrar.tool === "unar") {
          await execFileAsync(unrar.file, [...unrar.prefixArgs, "-force-overwrite", "-output-directory", destDir, archivePath], {
            timeout: 90_000,
            windowsHide: true,
            env: standaloneEnvironment(),
          });
          return { ok: true, method: "unar" };
        }
        if (unrar.tool === "7z") {
          await execFileAsync(unrar.file, [...unrar.prefixArgs, "x", "-y", `-o${destDir}`, archivePath], {
            timeout: 90_000,
            windowsHide: true,
            env: standaloneEnvironment(),
          });
          return { ok: true, method: "7z" };
        }
      } catch (err) {
        errors.push(`${unrar.tool}: ${err?.stderr || err?.message || err}`);
      }
    }

    // Thử bsdtar (Windows 10/11 tar.exe và macOS /usr/bin/tar đều tích hợp libarchive đọc được RAR)
    const tarBin = IS_WIN ? "C:\\Windows\\System32\\tar.exe" : "tar";
    try {
      await execFileAsync(tarBin, ["-xf", archivePath, "-C", destDir], {
        timeout: 90_000,
        windowsHide: true,
        env: standaloneEnvironment(),
      });
      return { ok: true, method: "bsdtar" };
    } catch (err) {
      errors.push(`bsdtar: ${err?.stderr || err?.message || err}`);
    }

    throw new Error(
      `Không thể giải nén tệp RAR (${errors.join(" | ")}). Hãy kiểm tra tệp RAR không đặt mật khẩu hoặc cài WinRAR / 7-Zip / unar.`
    );
  }

  // 2) Với ZIP: Thử giải nén thuần Node.js trước cho nhanh & kiểm soát Zip-Slip chặt, nếu gặp kiểu nén lạ thì fallback sang bsdtar / PowerShell
  if (isZip) {
    try {
      extractZipPureNode(archivePath, destDir);
      return { ok: true, method: "node-zlib" };
    } catch (err) {
      errors.push(`node-zlib: ${err?.message || err}`);
    }
  }

  // 3) Thử bsdtar cho .zip / .tar.gz / .7z
  const tarBin = IS_WIN && fs.existsSync("C:\\Windows\\System32\\tar.exe") ? "C:\\Windows\\System32\\tar.exe" : "tar";
  try {
    await execFileAsync(tarBin, ["-xf", archivePath, "-C", destDir], {
      timeout: 90_000,
      windowsHide: true,
      env: standaloneEnvironment(),
    });
    return { ok: true, method: "bsdtar" };
  } catch (err) {
    errors.push(`tar: ${err?.stderr || err?.message || err}`);
  }

  if (IS_WIN && isZip) {
    try {
      await execFileAsync(
        "powershell.exe",
        [
          "-NoProfile",
          "-NonInteractive",
          "-Command",
          `Expand-Archive -LiteralPath '${archivePath.replace(/'/g, "''")}' -DestinationPath '${destDir.replace(/'/g, "''")}' -Force`,
        ],
        { timeout: 90_000, windowsHide: true }
      );
      return { ok: true, method: "Expand-Archive" };
    } catch (err) {
      errors.push(`Expand-Archive: ${err?.stderr || err?.message || err}`);
    }
  }

  throw new Error(`Không thể giải nén tệp (${path.basename(archivePath)}): ${errors.join(" | ")}`);
}

/** Tìm tất cả thư mục Skill bên trong thư mục vừa giải nén / tải lên */
function detectSkillUnits(stagingDir, fallbackName) {
  const cleanEntries = fs
    .readdirSync(stagingDir, { withFileTypes: true })
    .filter((e) => !e.name.startsWith("__MACOSX") && e.name !== ".DS_Store");

  // TH1: Ngay gốc có SKILL.md
  if (fs.existsSync(path.join(stagingDir, "SKILL.md"))) {
    return [{ slug: sanitizeSlug(fallbackName), dir: stagingDir, autoGeneratedMd: false }];
  }

  // TH2: Các thư mục con cấp 1 có SKILL.md (hỗ trợ gói nhiều skill cùng lúc trong 1 file nén!)
  const directDirs = cleanEntries.filter((e) => e.isDirectory());
  const withMd = directDirs.filter((d) => fs.existsSync(path.join(stagingDir, d.name, "SKILL.md")));
  if (withMd.length > 0) {
    return withMd.map((d) => ({
      slug: sanitizeSlug(d.name),
      dir: path.join(stagingDir, d.name),
      autoGeneratedMd: false,
    }));
  }

  // TH3: Gói nén có 1 thư mục bao ngoài duy nhất, bên trong chứa các thư mục skill con (cấp 2)
  if (directDirs.length === 1) {
    const innerRoot = path.join(stagingDir, directDirs[0].name);
    const innerEntries = fs
      .readdirSync(innerRoot, { withFileTypes: true })
      .filter((e) => e.isDirectory() && !e.name.startsWith(".") && !e.name.startsWith("_"));
    const innerWithMd = innerEntries.filter((d) => fs.existsSync(path.join(innerRoot, d.name, "SKILL.md")));
    if (innerWithMd.length > 0) {
      return innerWithMd.map((d) => ({
        slug: sanitizeSlug(d.name),
        dir: path.join(innerRoot, d.name),
        autoGeneratedMd: false,
      }));
    }
    // Nếu 1 thư mục con duy nhất nhưng chưa có SKILL.md -> tạo SKILL.md chuẩn cho thư mục đó
    return [
      {
        slug: sanitizeSlug(directDirs[0].name || fallbackName),
        dir: innerRoot,
        autoGeneratedMd: true,
      },
    ];
  }

  // TH4: Các tệp đặt trực tiếp ở gốc nhưng chưa có SKILL.md -> tự sinh SKILL.md chuẩn
  if (cleanEntries.length > 0) {
    return [
      {
        slug: sanitizeSlug(fallbackName),
        dir: stagingDir,
        autoGeneratedMd: true,
      },
    ];
  }

  throw new Error("Thư mục hoặc tệp nén trống, không có dữ liệu Skill để nhập.");
}

function ensureSkillMarkdown(skillDir, slug) {
  const mdPath = path.join(skillDir, "SKILL.md");
  if (fs.existsSync(mdPath)) return;
  let extraDoc = "";
  for (const candidate of ["README.md", "prompt.md", "instructions.md", "config.md"]) {
    const p = path.join(skillDir, candidate);
    if (fs.existsSync(p)) {
      extraDoc = fs.readFileSync(p, "utf8").slice(0, 4000);
      break;
    }
  }
  const content = [
    "---",
    `name: ${slug}`,
    `description: Skill mở rộng "${slug}" được nhập vào Doto Skill Studio (hỗ trợ Antigravity, Claude Code và Codex).`,
    "---",
    "",
    `# Skill: ${slug}`,
    "",
    extraDoc || "Thực hiện yêu cầu sản xuất nội dung / video theo hướng dẫn và tài nguyên trong thư mục skill này.",
    "",
  ].join("\n");
  fs.writeFileSync(mdPath, content, "utf8");
}

/** Tạo liên kết thư mục (junction trên Windows / symlink trên macOS/Linux) để cả 3 AI nhận diện ngay Skill mới */
export function syncSkillLinksToAgents(skillId, skillDir) {
  const linked = [];
  const agentSkillDirs = [
    path.join(HOME, ".claude", "skills"),
    path.join(HOME, ".codex", "skills"),
    path.join(HOME, ".gemini", "skills"),
    path.join(HOME, ".gemini", "antigravity-cli", "skills"),
  ];
  for (const parentDir of agentSkillDirs) {
    try {
      fs.mkdirSync(parentDir, { recursive: true });
      const linkPath = path.join(parentDir, skillId);
      if (fs.existsSync(linkPath)) {
        const st = fs.lstatSync(linkPath);
        if (st.isSymbolicLink()) {
          fs.rmSync(linkPath, { recursive: true, force: true });
        } else {
          continue; // Không ghi đè thư mục thật đã tồn tại
        }
      }
      fs.symlinkSync(skillDir, linkPath, IS_WIN ? "junction" : "dir");
      linked.push(linkPath);
    } catch {
      // Không chặn luồng chính nếu HĐH hạn chế quyền tạo symlink
    }
  }
  return linked;
}

function installDetectedUnits(units, { targetRoot, overwrite = true, syncLinks = true, customSkillName = "" }) {
  const resolvedRoot = path.resolve(targetRoot);
  fs.mkdirSync(resolvedRoot, { recursive: true });
  const installed = [];

  for (let i = 0; i < units.length; i++) {
    const u = units[i];
    const finalSlug = units.length === 1 && customSkillName.trim() ? sanitizeSlug(customSkillName) : u.slug;
    const destSkillDir = path.join(resolvedRoot, finalSlug);

    if (fs.existsSync(destSkillDir)) {
      if (!overwrite) {
        throw new Error(`Skill "${finalSlug}" đã tồn tại tại ${destSkillDir}. Hãy bật tuỳ chọn Ghi đè hoặc đổi tên.`);
      }
      if (path.resolve(u.dir) !== path.resolve(destSkillDir)) {
        fs.rmSync(destSkillDir, { recursive: true, force: true });
      }
    }

    if (path.resolve(u.dir) !== path.resolve(destSkillDir)) {
      fs.cpSync(u.dir, destSkillDir, { recursive: true });
    }
    ensureSkillMarkdown(destSkillDir, finalSlug);
    const linkedPaths = syncLinks ? syncSkillLinksToAgents(finalSlug, destSkillDir) : [];

    installed.push({
      id: finalSlug,
      skillDir: destSkillDir,
      autoGeneratedMd: u.autoGeneratedMd,
      linkedPaths,
    });
  }

  return {
    ok: true,
    targetRoot: resolvedRoot,
    installed,
  };
}

export async function importSkillFromArchiveFile({
  archivePath,
  originalName = "",
  targetRoot,
  overwrite = true,
  syncLinks = true,
  customSkillName = "",
}) {
  const tmpStaging = fs.mkdtempSync(path.join(os.tmpdir(), "doto-skill-import-"));
  try {
    const extResult = await extractArchiveFile(archivePath, tmpStaging);
    const units = detectSkillUnits(tmpStaging, customSkillName || originalName || path.basename(archivePath));
    const res = installDetectedUnits(units, { targetRoot, overwrite, syncLinks, customSkillName });
    return { ...res, extractMethod: extResult.method };
  } finally {
    fs.rmSync(tmpStaging, { recursive: true, force: true });
  }
}

export async function importSkillFromLocalPath({
  sourcePath,
  targetRoot,
  overwrite = true,
  syncLinks = true,
  customSkillName = "",
}) {
  if (!sourcePath || !fs.existsSync(sourcePath)) {
    throw new Error(`Không tìm thấy đường dẫn nguồn: ${sourcePath}`);
  }
  const st = fs.statSync(sourcePath);
  if (st.isFile()) {
    return importSkillFromArchiveFile({
      archivePath: sourcePath,
      originalName: path.basename(sourcePath),
      targetRoot,
      overwrite,
      syncLinks,
      customSkillName,
    });
  }
  if (st.isDirectory()) {
    const units = detectSkillUnits(sourcePath, customSkillName || path.basename(sourcePath));
    return installDetectedUnits(units, { targetRoot, overwrite, syncLinks, customSkillName });
  }
  throw new Error("Đường dẫn nguồn không hợp lệ.");
}

export async function importSkillFromUploadedFiles({
  files,
  folderName = "imported-skill",
  targetRoot,
  overwrite = true,
  syncLinks = true,
  customSkillName = "",
}) {
  if (!Array.isArray(files) || files.length === 0) {
    throw new Error("Không có tệp nào trong thư mục tải lên.");
  }
  const tmpStaging = fs.mkdtempSync(path.join(os.tmpdir(), "doto-skill-folder-"));
  try {
    const resolvedTmp = path.resolve(tmpStaging);
    for (const f of files) {
      const rel = String(f.relativePath || f.name || "").replace(/\\/g, "/").replace(/^\/+/, "");
      if (!rel || rel.includes("..")) continue;
      const dest = path.resolve(resolvedTmp, rel);
      const normDest = IS_WIN ? dest.toLowerCase() : dest;
      const normRoot = IS_WIN ? resolvedTmp.toLowerCase() + path.sep : resolvedTmp + path.sep;
      if (!normDest.startsWith(normRoot)) continue;
      fs.mkdirSync(path.dirname(dest), { recursive: true });
      const buf = Buffer.from(String(f.contentBase64 || ""), "base64");
      fs.writeFileSync(dest, buf);
    }
    const units = detectSkillUnits(tmpStaging, customSkillName || folderName);
    return installDetectedUnits(units, { targetRoot, overwrite, syncLinks, customSkillName });
  } finally {
    fs.rmSync(tmpStaging, { recursive: true, force: true });
  }
}

export function deleteCustomSkill(skillId, skillRoots = []) {
  if (!skillId || CORE_SKILL_IDS.includes(skillId)) {
    return { ok: false, error: "Không thể xoá 6 Skill cốt lõi mặc định của hệ thống." };
  }
  let removed = false;
  for (const r of skillRoots) {
    if (!r) continue;
    const candidate = path.join(r, skillId);
    if (fs.existsSync(candidate) && fs.existsSync(path.join(candidate, "SKILL.md"))) {
      fs.rmSync(candidate, { recursive: true, force: true });
      removed = true;
    }
  }
  // Gỡ symlink nếu trỏ vào skill vừa xoá
  for (const parentDir of [
    path.join(HOME, ".claude", "skills"),
    path.join(HOME, ".codex", "skills"),
    path.join(HOME, ".gemini", "skills"),
    path.join(HOME, ".gemini", "antigravity-cli", "skills"),
  ]) {
    const lk = path.join(parentDir, skillId);
    try {
      if (fs.lstatSync(lk).isSymbolicLink()) {
        fs.rmSync(lk, { recursive: true, force: true });
      }
    } catch {
      // ignore
    }
  }
  return removed
    ? { ok: true, deletedId: skillId }
    : { ok: false, error: `Không tìm thấy thư mục Skill "${skillId}" để xoá.` };
}

/** Mở hộp thoại chọn Thư mục hoặc Tệp nén gốc trên máy chủ (Windows / macOS Desktop) */
export async function pickNativePath({ kind = "folder", title = "Chọn đường dẫn" } = {}) {
  try {
    if (IS_WIN) {
      const psScript =
        kind === "archive"
          ? `Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = '${title.replace(/'/g, "''")}'; $d.Filter = 'Skill Archives (*.zip;*.rar;*.7z;*.tar.gz)|*.zip;*.rar;*.7z;*.gz|All Files (*.*)|*.*'; if ($d.ShowDialog() -eq 'OK') { Write-Output $d.FileName }`
          : `Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = '${title.replace(/'/g, "''")}'; $d.ShowNewFolderButton = $true; if ($d.ShowDialog() -eq 'OK') { Write-Output $d.SelectedPath }`;
      const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-STA", "-Command", psScript], {
        timeout: 120_000,
        windowsHide: false,
      });
      const picked = String(stdout || "").trim();
      return { ok: Boolean(picked), path: picked };
    }
    if (process.platform === "darwin") {
      const osa =
        kind === "archive"
          ? `POSIX path of (choose file with prompt "${title.replace(/"/g, '\\"')}")`
          : `POSIX path of (choose folder with prompt "${title.replace(/"/g, '\\"')}")`;
      const { stdout } = await execFileAsync("osascript", ["-e", osa], { timeout: 120_000 });
      const picked = String(stdout || "").trim();
      return { ok: Boolean(picked), path: picked };
    }
    return { ok: false, error: "Hộp thoại hệ điều hành chỉ hỗ trợ trực tiếp trên Windows / macOS Desktop." };
  } catch {
    return { ok: false, path: "" };
  }
}

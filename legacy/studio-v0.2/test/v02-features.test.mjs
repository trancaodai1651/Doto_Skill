import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { startServer } from "../server/index.mjs";
import { composeSkillPrompt } from "../server/skills.mjs";

test("v0.2.0 End-to-End: Import Skill (.zip, .rar, Folder), Custom Save Dir, Mobile Photos Upload, Tunnel CRUD & Shared Memory", async () => {
  const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), "doto-v02-test-"));
  const dataDir = path.join(tmpBase, "data");
  const customSkillDir = path.join(tmpBase, "my-custom-skills");
  const customVideoDir = path.join(tmpBase, "my-videos");
  const customMediaDir = path.join(tmpBase, "my-photos");
  const memWorkspace = path.join(tmpBase, "shared-ai-workspace");

  process.env.DOTO_SKILL_DATA_DIR = dataDir;

  const { server } = await startServer({ port: 0, host: "127.0.0.1" });
  const port = server.address().port;
  const baseUrl = `http://127.0.0.1:${port}`;

  try {
    // 1. Lưu cấu hình nơi lưu tuỳ chỉnh, giao diện Light & ngôn ngữ EN
    const saveRes = await (
      await fetch(`${baseUrl}/api/settings`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          customSkillSaveDir: customSkillDir,
          videoOutputDir: customVideoDir,
          mediaUploadDir: customMediaDir,
          theme: "light",
          language: "en",
        }),
      })
    ).json();
    assert.equal(saveRes.ok, true);
    assert.equal(saveRes.settings.theme, "light");
    assert.equal(saveRes.settings.language, "en");
    assert.equal(saveRes.settings.videoOutputDir, customVideoDir);

    // 2. Import Skill qua Browser Folder Upload vào nơi lưu tuỳ chỉnh
    const folderImport = await (
      await fetch(`${baseUrl}/api/skills/import-folder-upload`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          folderName: "tiktok-review-skill",
          targetDir: customSkillDir,
          syncLinks: false,
          files: [
            {
              name: "SKILL.md",
              relativePath: "tiktok-review-skill/SKILL.md",
              contentBase64: Buffer.from("---\nname: TikTok Review 60s\ndescription: Review sản phẩm nhanh\n---\n# TikTok Review", "utf8").toString("base64"),
            },
          ],
        }),
      })
    ).json();
    assert.equal(folderImport.ok, true);
    assert.ok(folderImport.skillsInfo.skills.some((s) => s.id === "tiktok-review-skill"));

    // 3. Tạo file .ZIP thật và Import qua /api/skills/import-archive
    const zipSrcDir = path.join(tmpBase, "zip-src", "zip-imported-skill");
    fs.mkdirSync(zipSrcDir, { recursive: true });
    fs.writeFileSync(path.join(zipSrcDir, "SKILL.md"), "---\nname: Zip Imported\ndescription: Skill từ ZIP\n---\n# Hello ZIP", "utf8");
    const zipFile = path.join(tmpBase, "zip-skill.zip");
    const tarExe = process.platform === "win32" ? "C:\\Windows\\System32\\tar.exe" : "tar";
    execFileSync(tarExe, ["-a", "-cf", zipFile, "-C", path.join(tmpBase, "zip-src"), "zip-imported-skill"]);

    const zipBytes = fs.readFileSync(zipFile);
    const zipImport = await (
      await fetch(`${baseUrl}/api/skills/import-archive`, {
        method: "POST",
        headers: {
          "Content-Type": "application/octet-stream",
          "X-File-Name": "zip-skill.zip",
          "X-Target-Dir": encodeURIComponent(customSkillDir),
          "X-Sync-Links": "0",
        },
        body: zipBytes,
      })
    ).json();
    assert.equal(zipImport.ok, true);
    assert.ok(zipImport.skillsInfo.skills.some((s) => s.id === "zip-imported-skill"));

    // 4. Nếu máy có WinRAR.exe, kiểm tra nén & giải nén .RAR thật 100%!
    const winRarExe = "C:\\Program Files\\WinRAR\\WinRAR.exe";
    if (process.platform === "win32" && fs.existsSync(winRarExe)) {
      const rarSrcDir = path.join(tmpBase, "rar-src", "rar-imported-skill");
      fs.mkdirSync(rarSrcDir, { recursive: true });
      fs.writeFileSync(path.join(rarSrcDir, "SKILL.md"), "---\nname: RAR Imported\ndescription: Skill từ RAR\n---\n# Hello RAR", "utf8");
      const rarFile = path.join(tmpBase, "rar-skill.rar");
      execFileSync(winRarExe, ["a", "-ep1", "-ibck", "-y", rarFile, rarSrcDir]);

      const rarBytes = fs.readFileSync(rarFile);
      const rarImport = await (
        await fetch(`${baseUrl}/api/skills/import-archive`, {
          method: "POST",
          headers: {
            "Content-Type": "application/octet-stream",
            "X-File-Name": "rar-skill.rar",
            "X-Target-Dir": encodeURIComponent(customSkillDir),
            "X-Sync-Links": "0",
          },
          body: rarBytes,
        })
      ).json();
      assert.equal(rarImport.ok, true);
      assert.ok(rarImport.skillsInfo.skills.some((s) => s.id === "rar-imported-skill"));
    }

    // 5. Xoá Skill mở rộng nhưng chặn xoá Skill cốt lõi
    const delCore = await (await fetch(`${baseUrl}/api/skills/news-video`, { method: "DELETE" })).json();
    assert.equal(delCore.ok, false);

    const delCustom = await (await fetch(`${baseUrl}/api/skills/tiktok-review-skill`, { method: "DELETE" })).json();
    assert.equal(delCustom.ok, true);

    // 6. Tải ảnh từ App Photos di động (/api/media-upload) & tải về kèm Content-Disposition
    const fakePhoto = Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46]);
    const mediaRes = await (
      await fetch(`${baseUrl}/api/media-upload`, {
        method: "POST",
        headers: {
          "Content-Type": "image/jpeg",
          "X-File-Name": encodeURIComponent("IMG_2026_iPhone.JPG"),
        },
        body: fakePhoto,
      })
    ).json();
    assert.equal(mediaRes.ok, true);
    assert.equal(mediaRes.file.kind, "image");

    const dlResp = await fetch(`${baseUrl}/api/media?path=${encodeURIComponent(mediaRes.file.path)}&download=1`);
    assert.equal(dlResp.status, 200);
    assert.match(dlResp.headers.get("content-disposition") || "", /attachment/i);

    // 7. Quản lý CRUD Cloudflare Tunnel (Create -> Read -> Update -> Delete)
    const createTun = await (
      await fetch(`${baseUrl}/api/tunnels`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: "Test Web Tunnel",
          mode: "quick",
          targetUrl: "http://127.0.0.1:4320",
          protocol: "http2",
          startNow: false,
        }),
      })
    ).json();
    assert.equal(createTun.ok, true);
    const tunId = createTun.tunnel.id;

    const updTun = await (
      await fetch(`${baseUrl}/api/tunnels/${tunId}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: "Updated Web Tunnel", autoStart: true }),
      })
    ).json();
    assert.equal(updTun.ok, true);
    assert.equal(updTun.tunnel.name, "Updated Web Tunnel");
    assert.equal(updTun.tunnel.autoStart, true);

    const delTun = await (await fetch(`${baseUrl}/api/tunnels/${tunId}`, { method: "DELETE" })).json();
    assert.equal(delTun.ok, true);

    // 8. Bộ nhớ chung & Bàn giao cho Antigravity, Claude, Codex
    const initMem = await (
      await fetch(`${baseUrl}/api/memory/init`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ workspace: memWorkspace }),
      })
    ).json();
    assert.equal(initMem.ok, true);
    assert.equal(initMem.status.files.filter((f) => f.exists).length, 5);

    const appendMem = await (
      await fetch(`${baseUrl}/api/memory/append-note`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ workspace: memWorkspace, agent: "Antigravity", note: "Đã kiểm chứng quy trình đa phiên" }),
      })
    ).json();
    assert.equal(appendMem.ok, true);
    const memDoc = await (
      await fetch(`${baseUrl}/api/memory/doc?workspace=${encodeURIComponent(memWorkspace)}&file=MEMORY.md`)
    ).json();
    assert.match(memDoc.content, /Đã kiểm chứng quy trình đa phiên/);

    // 9. Kiểm tra ghép câu lệnh có thư mục lưu tuỳ chỉnh + media đính kèm + bộ nhớ chung
    const promptWithAll = composeSkillPrompt({
      skillId: "product-ad-video",
      providerId: "antigravity",
      brief: "Quảng cáo tai nghe",
      outputDir: customVideoDir,
      projectFolder: "2026-11-26-ad",
      mediaFiles: [mediaRes.file.path],
      sharedMemoryHint: "Đọc MEMORY.md và HANDOFF.md.",
    });
    assert.ok(promptWithAll.startsWith("/product-ad-video "));
    assert.ok(promptWithAll.includes(customVideoDir));
    assert.ok(promptWithAll.includes(mediaRes.file.path));
    assert.ok(promptWithAll.includes("HANDOFF.md"));
  } finally {
    server.close();
    delete process.env.DOTO_SKILL_DATA_DIR;
    fs.rmSync(tmpBase, { recursive: true, force: true });
  }
});

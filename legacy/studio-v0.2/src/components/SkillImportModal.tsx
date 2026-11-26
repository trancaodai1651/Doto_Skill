import React, { useRef, useState } from "react";
import { Upload, FolderUp, HardDrive, X, FolderOpen, CheckCircle2, AlertCircle } from "lucide-react";
import type { LangCode } from "../types";

interface Props {
  open: boolean;
  lang: LangCode;
  defaultTargetDir: string;
  skillRoot: string;
  defaultSyncLinks: boolean;
  onClose: () => void;
  onImported: () => void;
}

type ImportMode = "archive" | "folder-upload" | "local-path";

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export const SkillImportModal: React.FC<Props> = ({
  open,
  lang,
  defaultTargetDir,
  skillRoot,
  defaultSyncLinks,
  onClose,
  onImported,
}) => {
  const vi = lang === "vi";
  const [mode, setMode] = useState<ImportMode>("archive");
  const [targetDir, setTargetDir] = useState<string>(defaultTargetDir || skillRoot);
  const [customSkillName, setCustomSkillName] = useState<string>("");
  const [sourcePath, setSourcePath] = useState<string>("");
  const [overwrite, setOverwrite] = useState<boolean>(true);
  const [syncLinks, setSyncLinks] = useState<boolean>(defaultSyncLinks);
  const [archiveFile, setArchiveFile] = useState<File | null>(null);
  const [folderFiles, setFolderFiles] = useState<FileList | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [errMsg, setErrMsg] = useState<string>("");
  const [okMsg, setOkMsg] = useState<string>("");

  const archiveInputRef = useRef<HTMLInputElement | null>(null);
  const folderInputRef = useRef<HTMLInputElement | null>(null);

  if (!open) return null;

  const pickOsPath = async (kind: "folder" | "archive", setter: (v: string) => void) => {
    try {
      const res = await fetch("/api/pick-path", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind,
          title: vi ? "Chọn đường dẫn Skill" : "Select Skill Path",
        }),
      });
      const j = await res.json();
      if (j.ok && j.path) setter(j.path);
    } catch {
      // ignore on remote mobile
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrMsg("");
    setOkMsg("");
    setBusy(true);

    try {
      if (mode === "archive") {
        if (!archiveFile) {
          throw new Error(vi ? "Vui lòng chọn tệp nén (.zip, .rar, .7z, .tar.gz)." : "Please select a .zip or .rar archive file.");
        }
        const res = await fetch("/api/skills/import-archive", {
          method: "POST",
          headers: {
            "Content-Type": "application/octet-stream",
            "X-File-Name": encodeURIComponent(archiveFile.name),
            "X-Target-Dir": encodeURIComponent(targetDir.trim()),
            "X-Overwrite": overwrite ? "1" : "0",
            "X-Sync-Links": syncLinks ? "1" : "0",
            "X-Skill-Name": encodeURIComponent(customSkillName.trim()),
          },
          body: archiveFile,
        });
        const j = await res.json();
        if (!j.ok) throw new Error(j.error || "Import failed");
        const ids = (j.installed || []).map((x: any) => x.id).join(", ");
        setOkMsg(
          vi
            ? `Đã nhập thành công Skill [${ids}] vào "${j.targetRoot}"!`
            : `Successfully imported Skill [${ids}] into "${j.targetRoot}"!`
        );
        onImported();
      } else if (mode === "folder-upload") {
        if (!folderFiles || folderFiles.length === 0) {
          throw new Error(vi ? "Vui lòng chọn một thư mục Skill từ thiết bị." : "Please select a Skill folder.");
        }
        const serialized = [];
        let topFolder = "imported-skill";
        for (let i = 0; i < folderFiles.length; i++) {
          const f = folderFiles[i];
          const rel = (f as any).webkitRelativePath || f.name;
          if (i === 0 && rel.includes("/")) {
            topFolder = rel.split("/")[0];
          }
          if (rel.includes("node_modules/") || rel.includes(".git/")) continue;
          const buf = await f.arrayBuffer();
          serialized.push({
            name: f.name,
            relativePath: rel,
            contentBase64: arrayBufferToBase64(buf),
          });
        }
        const res = await fetch("/api/skills/import-folder-upload", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            files: serialized,
            folderName: topFolder,
            targetDir: targetDir.trim(),
            overwrite,
            syncLinks,
            customSkillName: customSkillName.trim(),
          }),
        });
        const j = await res.json();
        if (!j.ok) throw new Error(j.error || "Folder import failed");
        const ids = (j.installed || []).map((x: any) => x.id).join(", ");
        setOkMsg(
          vi
            ? `Đã tải lên & cài đặt Skill [${ids}] tại "${j.targetRoot}"!`
            : `Uploaded & installed Skill [${ids}] at "${j.targetRoot}"!`
        );
        onImported();
      } else {
        if (!sourcePath.trim()) {
          throw new Error(vi ? "Hãy nhập hoặc chọn đường dẫn thư mục / file .zip / .rar trên máy." : "Enter or browse a local folder or .zip/.rar path.");
        }
        const res = await fetch("/api/skills/import-local", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            sourcePath: sourcePath.trim(),
            targetDir: targetDir.trim(),
            overwrite,
            syncLinks,
            customSkillName: customSkillName.trim(),
          }),
        });
        const j = await res.json();
        if (!j.ok) throw new Error(j.error || "Local import failed");
        const ids = (j.installed || []).map((x: any) => x.id).join(", ");
        setOkMsg(
          vi
            ? `Đã nhập Skill [${ids}] vào "${j.targetRoot}"!`
            : `Imported Skill [${ids}] into "${j.targetRoot}"!`
        );
        onImported();
      }
    } catch (err: any) {
      setErrMsg(err?.message || String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <form className="modal-card" onClick={(e) => e.stopPropagation()} onSubmit={handleSubmit}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10 }}>
          <div>
            <h3 style={{ margin: 0, fontSize: 17 }}>
              {vi ? "Nhập (Import) Skill vào Ứng Dụng" : "Import Skill into Studio"}
            </h3>
            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>
              {vi
                ? "Hỗ trợ tệp nén (.zip, .rar, .7z, .tar.gz) hoặc cả thư mục Skill, tùy chỉnh nơi lưu & tự gắn kết 3 AI"
                : "Import from .zip / .rar archive or folder, customize save location & auto-link to all 3 AIs"}
            </div>
          </div>
          <button type="button" className="btn-secondary" onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 6 }}>
          <button
            type="button"
            className={`mode-pill ${mode === "archive" ? "active" : ""}`}
            onClick={() => setMode("archive")}
          >
            <Upload size={13} style={{ verticalAlign: "middle", marginRight: 4 }} />
            {vi ? "File nén (.zip / .rar)" : "Archive (.zip / .rar)"}
          </button>
          <button
            type="button"
            className={`mode-pill ${mode === "folder-upload" ? "active" : ""}`}
            onClick={() => setMode("folder-upload")}
          >
            <FolderUp size={13} style={{ verticalAlign: "middle", marginRight: 4 }} />
            {vi ? "Tải Thư mục lên" : "Upload Folder"}
          </button>
          <button
            type="button"
            className={`mode-pill ${mode === "local-path" ? "active" : ""}`}
            onClick={() => setMode("local-path")}
          >
            <HardDrive size={13} style={{ verticalAlign: "middle", marginRight: 4 }} />
            {vi ? "Đường dẫn trên PC" : "Local PC Path"}
          </button>
        </div>

        {mode === "archive" && (
          <div className="photos-box" style={{ alignItems: "center", textAlign: "center", padding: 18 }}>
            <input
              ref={archiveInputRef}
              type="file"
              accept=".zip,.rar,.7z,.tar.gz,.tgz,.tar,application/zip,application/x-rar-compressed,application/vnd.rar"
              style={{ display: "none" }}
              onChange={(e) => setArchiveFile(e.target.files?.[0] || null)}
            />
            <Upload size={26} color="var(--accent)" />
            <div style={{ fontSize: 13.5, fontWeight: 700 }}>
              {archiveFile
                ? `${archiveFile.name} (${(archiveFile.size / 1024).toFixed(1)} KB)`
                : vi
                  ? "Chọn tệp nén Skill (.zip hoặc .rar)"
                  : "Choose Skill archive (.zip or .rar)"}
            </div>
            <button type="button" className="btn-secondary" onClick={() => archiveInputRef.current?.click()}>
              {vi ? "Chọn tệp .ZIP / .RAR từ thiết bị" : "Pick .ZIP / .RAR File"}
            </button>
          </div>
        )}

        {mode === "folder-upload" && (
          <div className="photos-box" style={{ alignItems: "center", textAlign: "center", padding: 18 }}>
            <input
              ref={folderInputRef}
              type="file"
              multiple
              {...({ webkitdirectory: "", directory: "" } as any)}
              style={{ display: "none" }}
              onChange={(e) => setFolderFiles(e.target.files)}
            />
            <FolderUp size={26} color="var(--accent)" />
            <div style={{ fontSize: 13.5, fontWeight: 700 }}>
              {folderFiles && folderFiles.length > 0
                ? vi
                  ? `Đã chọn thư mục (${folderFiles.length} tệp)`
                  : `Selected folder (${folderFiles.length} files)`
                : vi
                  ? "Chọn nguyên thư mục chứa Skill từ máy tính / iPad"
                  : "Select entire Skill folder from device"}
            </div>
            <button type="button" className="btn-secondary" onClick={() => folderInputRef.current?.click()}>
              {vi ? "Duyệt chọn Thư mục Skill" : "Select Skill Folder"}
            </button>
          </div>
        )}

        {mode === "local-path" && (
          <div className="field">
            <label>{vi ? "Đường dẫn Thư mục hoặc Tệp .zip / .rar trên máy chủ:" : "Local Folder or .zip / .rar Path:"}</label>
            <div className="input-with-btn">
              <input
                className="input"
                value={sourcePath}
                onChange={(e) => setSourcePath(e.target.value)}
                placeholder="D:\Downloads\my-skill.rar hoặc D:\MySkills\custom-video"
              />
              <button type="button" className="btn-secondary" onClick={() => pickOsPath("folder", setSourcePath)}>
                <FolderOpen size={14} />
                <span>{vi ? "Thư mục" : "Folder"}</span>
              </button>
              <button type="button" className="btn-secondary" onClick={() => pickOsPath("archive", setSourcePath)}>
                <Upload size={14} />
                <span>.zip/.rar</span>
              </button>
            </div>
          </div>
        )}

        <div className="field">
          <label>{vi ? "Nơi lưu Skill sau khi Import (Tùy chỉnh thư mục đích):" : "Custom Destination Folder to Save Skill:"}</label>
          <div className="input-with-btn">
            <input
              className="input"
              value={targetDir}
              onChange={(e) => setTargetDir(e.target.value)}
              placeholder={skillRoot}
            />
            <button type="button" className="btn-secondary" onClick={() => pickOsPath("folder", setTargetDir)}>
              <FolderOpen size={14} />
              <span>{vi ? "Chọn nơi lưu" : "Browse"}</span>
            </button>
          </div>
          {targetDir !== skillRoot && (
            <div style={{ marginTop: 4 }}>
              <button type="button" className="chip" onClick={() => setTargetDir(skillRoot)}>
                {vi ? `Đặt về thư mục gốc (${skillRoot})` : `Reset to default (${skillRoot})`}
              </button>
            </div>
          )}
        </div>

        <div className="field">
          <label>{vi ? "Đặt mã tên Skill (Tùy chọn — để trống sẽ tự lấy theo tên thư mục/file):" : "Custom Skill Slug (Optional):"}</label>
          <input
            className="input"
            value={customSkillName}
            onChange={(e) => setCustomSkillName(e.target.value)}
            placeholder="vi-du: review-phim-60s"
          />
        </div>

        <div style={{ display: "flex", flexWrap: "wrap", gap: 16, fontSize: 12.5 }}>
          <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}>
            <input type="checkbox" checked={overwrite} onChange={(e) => setOverwrite(e.target.checked)} />
            <span>{vi ? "Ghi đè nếu Skill trùng tên đã tồn tại" : "Overwrite if skill already exists"}</span>
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}>
            <input type="checkbox" checked={syncLinks} onChange={(e) => setSyncLinks(e.target.checked)} />
            <span>
              {vi
                ? "Tự động liên kết (junction) vào Antigravity, Claude Code & Codex"
                : "Auto-link junction into Antigravity, Claude Code & Codex"}
            </span>
          </label>
        </div>

        {errMsg && (
          <div style={{ color: "var(--red-text)", fontSize: 12.5, display: "flex", alignItems: "center", gap: 6 }}>
            <AlertCircle size={15} />
            <span>{errMsg}</span>
          </div>
        )}
        {okMsg && (
          <div style={{ color: "var(--green-text)", fontSize: 12.5, display: "flex", alignItems: "center", gap: 6 }}>
            <CheckCircle2 size={15} />
            <span>{okMsg}</span>
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button type="button" className="btn-secondary" onClick={onClose}>
            {vi ? "Đóng" : "Close"}
          </button>
          <button type="submit" className="btn-primary" style={{ width: "auto", minWidth: 180 }} disabled={busy}>
            {busy ? (vi ? "Đang giải nén & nhập..." : "Importing...") : vi ? "Nhập Skill Ngay" : "Import Skill Now"}
          </button>
        </div>
      </form>
    </div>
  );
};

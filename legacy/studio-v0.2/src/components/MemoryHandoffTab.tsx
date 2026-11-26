import React, { useEffect, useState } from "react";
import { BookOpen, Save, RefreshCw, FolderOpen, PlusCircle, CheckCircle2 } from "lucide-react";
import type { LangCode, MemoryHandoffStatus } from "../types";
import { UI_TEXT } from "../i18n";

interface Props {
  lang: LangCode;
  skillRoot: string;
  appRoot: string;
  initialStatus: MemoryHandoffStatus;
  onSynced: () => void;
}

export const MemoryHandoffTab: React.FC<Props> = ({
  lang,
  skillRoot,
  appRoot,
  initialStatus,
  onSynced,
}) => {
  const t = UI_TEXT[lang];
  const vi = lang === "vi";

  const [workspace, setWorkspace] = useState<string>(skillRoot);
  const [status, setStatus] = useState<MemoryHandoffStatus>(initialStatus);
  const [selectedFile, setSelectedFile] = useState<string>("MEMORY.md");
  const [content, setContent] = useState<string>("");
  const [loadingDoc, setLoadingDoc] = useState<boolean>(false);
  const [saving, setSaving] = useState<boolean>(false);
  const [savedToast, setSavedToast] = useState<string>("");
  const [noteAgent, setNoteAgent] = useState<string>("Antigravity");
  const [quickNote, setQuickNote] = useState<string>("");

  const loadWorkspaceStatus = async (ws: string, fileToLoad = selectedFile) => {
    setLoadingDoc(true);
    try {
      const [stRes, docRes] = await Promise.all([
        fetch(`/api/memory/status?workspace=${encodeURIComponent(ws)}`),
        fetch(`/api/memory/doc?workspace=${encodeURIComponent(ws)}&file=${encodeURIComponent(fileToLoad)}`),
      ]);
      const stJson = await stRes.json();
      const docJson = await docRes.json();
      if (stJson.ok && stJson.status) setStatus(stJson.status);
      if (docJson.ok) setContent(docJson.content || "");
    } finally {
      setLoadingDoc(false);
    }
  };

  useEffect(() => {
    loadWorkspaceStatus(workspace, selectedFile);
  }, [workspace, selectedFile]);

  const handleInitAll = async () => {
    setSaving(true);
    try {
      await fetch("/api/memory/init", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ workspace, overwrite: false }),
      });
      await loadWorkspaceStatus(workspace, selectedFile);
      onSynced();
      setSavedToast(vi ? "Đã đồng bộ đủ 5 tệp chuẩn cho Antigravity, Claude & Codex!" : "Synced all 5 files for Antigravity, Claude & Codex!");
      setTimeout(() => setSavedToast(""), 2800);
    } finally {
      setSaving(false);
    }
  };

  const handleSaveFile = async () => {
    setSaving(true);
    try {
      const res = await fetch("/api/memory/doc", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ workspace, file: selectedFile, content }),
      });
      const j = await res.json();
      if (j.ok && j.status) setStatus(j.status);
      setSavedToast(vi ? `Đã lưu ${selectedFile}!` : `Saved ${selectedFile}!`);
      setTimeout(() => setSavedToast(""), 2200);
    } finally {
      setSaving(false);
    }
  };

  const handleAppendMemory = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!quickNote.trim()) return;
    setSaving(true);
    try {
      await fetch("/api/memory/append-note", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ workspace, agent: noteAgent, note: quickNote.trim() }),
      });
      setQuickNote("");
      setSelectedFile("MEMORY.md");
      await loadWorkspaceStatus(workspace, "MEMORY.md");
      setSavedToast(vi ? "Đã ghi thêm vào MEMORY.md!" : "Appended to MEMORY.md!");
      setTimeout(() => setSavedToast(""), 2200);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <section className="step-section" style={{ marginBottom: 0 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 12 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 18, display: "flex", alignItems: "center", gap: 8 }}>
              <BookOpen size={20} color="var(--accent)" />
              <span>{t.memoryHubTitle}</span>
            </h2>
            <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--text-muted)" }}>{t.memoryHubSub}</p>
          </div>

          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button type="button" className="btn-primary" style={{ width: "auto" }} disabled={saving} onClick={handleInitAll}>
              <RefreshCw size={15} />
              <span>{t.syncAllDocsBtn}</span>
            </button>
            <button
              type="button"
              className="btn-secondary"
              onClick={() =>
                fetch("/api/open-path", {
                  method: "POST",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({ path: workspace }),
                })
              }
            >
              <FolderOpen size={14} />
              <span>{t.openSkillDirBtn}</span>
            </button>
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 12, alignItems: "center" }}>
          <span style={{ fontSize: 12, fontWeight: 700, color: "var(--text-muted)" }}>
            {vi ? "Không gian bộ nhớ đang xem:" : "Target Workspace:"}
          </span>
          <button
            type="button"
            className={`chip ${workspace === skillRoot ? "active" : ""}`}
            onClick={() => setWorkspace(skillRoot)}
          >
            🎬 Bộ Skill ({skillRoot})
          </button>
          {appRoot && appRoot !== skillRoot && (
            <button
              type="button"
              className={`chip ${workspace === appRoot ? "active" : ""}`}
              onClick={() => setWorkspace(appRoot)}
            >
              💻 Doto Skill App ({appRoot})
            </button>
          )}
        </div>

        <form onSubmit={handleAppendMemory} style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
          <select
            className="select"
            style={{ maxWidth: 180 }}
            value={noteAgent}
            onChange={(e) => setNoteAgent(e.target.value)}
          >
            <option value="Antigravity">Google Antigravity</option>
            <option value="Claude Code">Claude Code</option>
            <option value="Codex">OpenAI Codex</option>
            <option value="User">Người dùng / Chung</option>
          </select>
          <input
            className="input"
            style={{ flex: 1, minWidth: 220 }}
            value={quickNote}
            onChange={(e) => setQuickNote(e.target.value)}
            placeholder={t.quickNotePlaceholder}
          />
          <button type="submit" className="btn-secondary" disabled={saving}>
            <PlusCircle size={14} />
            <span>{t.appendMemoryBtn}</span>
          </button>
        </form>

        {savedToast && (
          <div style={{ marginTop: 10, color: "var(--green-text)", fontSize: 13, display: "flex", alignItems: "center", gap: 6 }}>
            <CheckCircle2 size={15} />
            <span>{savedToast}</span>
          </div>
        )}
      </section>

      <div className="memory-layout">
        <div className="step-section" style={{ marginBottom: 0 }}>
          <h3 style={{ marginTop: 0, fontSize: 14.5 }}>
            {vi ? "Tệp Bộ nhớ & Bàn giao (5 tệp)" : "Shared Context Files (5)"}
          </h3>
          {status.files.map((f) => (
            <button
              key={f.name}
              type="button"
              className={`job-item ${selectedFile === f.name ? "active" : ""}`}
              onClick={() => setSelectedFile(f.name)}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <strong style={{ fontFamily: "var(--font-mono)", fontSize: 13.5 }}>{f.name}</strong>
                <span className={`dot-badge ${f.exists ? "ready" : "missing"}`}>
                  {f.exists ? `${(f.size / 1024).toFixed(1)} KB` : vi ? "Chưa tạo" : "Missing"}
                </span>
              </div>
              <div style={{ fontSize: 11.5, color: "var(--text-muted)", marginTop: 4 }}>
                {vi ? f.roleVi : f.roleEn}
              </div>
              <div style={{ display: "flex", gap: 4, marginTop: 6, flexWrap: "wrap" }}>
                {f.agents.map((ag) => (
                  <span key={ag} className="pill" style={{ fontSize: 10, padding: "2px 7px" }}>
                    {ag.toUpperCase()}
                  </span>
                ))}
              </div>
            </button>
          ))}
        </div>

        <div className="step-section" style={{ marginBottom: 0, display: "flex", flexDirection: "column", gap: 10 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: 8 }}>
            <div>
              <h3 style={{ margin: 0, fontSize: 15.5, fontFamily: "var(--font-mono)" }}>{selectedFile}</h3>
              <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
                <code>{workspace}\{selectedFile}</code>
              </div>
            </div>
            <button type="button" className="btn-primary" style={{ width: "auto" }} disabled={saving || loadingDoc} onClick={handleSaveFile}>
              <Save size={15} />
              <span>{t.saveDocBtn} ({selectedFile})</span>
            </button>
          </div>

          <textarea
            className="textarea"
            style={{ minHeight: 440, fontFamily: "var(--font-mono)", fontSize: 12.5, lineHeight: 1.55 }}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder={
              vi
                ? `Tệp ${selectedFile} chưa có nội dung. Bấm "Khởi tạo / Đồng bộ đủ 5 tệp chuẩn" hoặc nhập trực tiếp tại đây...`
                : `Edit ${selectedFile} here...`
            }
          />
        </div>
      </div>
    </div>
  );
};

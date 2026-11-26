import React, { useState } from "react";
import {
  Globe,
  Plus,
  Play,
  Square,
  RotateCw,
  Trash2,
  Edit3,
  Copy,
  Check,
  ExternalLink,
  Terminal,
  ShieldCheck,
  Smartphone,
} from "lucide-react";
import type { LangCode, ManagedTunnelItem, TunnelsOverview } from "../types";
import { UI_TEXT } from "../i18n";

interface Props {
  lang: LangCode;
  tunnels: TunnelsOverview;
  lanUrls: string[];
  onRefresh: () => void;
}

export const TunnelManagerTab: React.FC<Props> = ({ lang, tunnels, lanUrls, onRefresh }) => {
  const t = UI_TEXT[lang];
  const vi = lang === "vi";

  const [showForm, setShowForm] = useState<boolean>(false);
  const [editingId, setEditingId] = useState<string>("");
  const [name, setName] = useState<string>("Doto Skill Studio Web");
  const [mode, setMode] = useState<"quick" | "token" | "named">("quick");
  const [targetUrl, setTargetUrl] = useState<string>(tunnels.defaultWebUrl || "http://127.0.0.1:4320");
  const [token, setToken] = useState<string>("");
  const [tunnelNameOrId, setTunnelNameOrId] = useState<string>("");
  const [publicHostname, setPublicHostname] = useState<string>("");
  const [protocol, setProtocol] = useState<"auto" | "http2" | "quic">("http2");
  const [autoStart, setAutoStart] = useState<boolean>(true);
  const [busy, setBusy] = useState<boolean>(false);
  const [copiedId, setCopiedId] = useState<string>("");
  const [expandedLogsId, setExpandedLogsId] = useState<string>("");
  const [errMsg, setErrMsg] = useState<string>("");

  const resetForm = () => {
    setEditingId("");
    setName("Doto Skill Studio Web");
    setMode("quick");
    setTargetUrl(tunnels.defaultWebUrl || "http://127.0.0.1:4320");
    setToken("");
    setTunnelNameOrId("");
    setPublicHostname("");
    setProtocol("http2");
    setAutoStart(true);
  };

  const openCreate = () => {
    resetForm();
    setShowForm(true);
  };

  const openEdit = (item: ManagedTunnelItem) => {
    setEditingId(item.id);
    setName(item.name);
    setMode(item.mode);
    setTargetUrl(item.targetUrl);
    setToken("");
    setTunnelNameOrId(item.tunnelNameOrId || "");
    setPublicHostname(item.publicHostname || "");
    setProtocol(item.protocol || "auto");
    setAutoStart(item.autoStart);
    setShowForm(true);
  };

  const handleQuickWebTunnel = async () => {
    setBusy(true);
    setErrMsg("");
    try {
      const res = await fetch("/api/tunnels/quick-web", { method: "POST" });
      const j = await res.json();
      if (!j.ok) setErrMsg(j.error || "Failed to start Quick Tunnel");
      onRefresh();
    } finally {
      setBusy(false);
    }
  };

  const handleSaveTunnel = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErrMsg("");
    try {
      const url = editingId ? `/api/tunnels/${encodeURIComponent(editingId)}` : "/api/tunnels";
      const method = editingId ? "PUT" : "POST";
      const res = await fetch(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name,
          mode,
          targetUrl,
          token,
          tunnelNameOrId,
          publicHostname,
          protocol,
          autoStart,
          startNow: true,
        }),
      });
      const j = await res.json();
      if (!j.ok) {
        setErrMsg(j.error || "Tunnel save error");
        return;
      }
      setShowForm(false);
      resetForm();
      onRefresh();
    } finally {
      setBusy(false);
    }
  };

  const handleAction = async (id: string, action: "start" | "stop" | "restart" | "delete") => {
    setBusy(true);
    setErrMsg("");
    try {
      const res =
        action === "delete"
          ? await fetch(`/api/tunnels/${encodeURIComponent(id)}`, { method: "DELETE" })
          : await fetch(`/api/tunnels/${encodeURIComponent(id)}/${action}`, { method: "POST" });
      const j = await res.json();
      if (!j.ok) setErrMsg(j.error || "Action failed");
      onRefresh();
    } finally {
      setBusy(false);
    }
  };

  const handleAdopt = async (pid: number) => {
    await fetch("/api/tunnels/adopt-external", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ pid }),
    });
    onRefresh();
  };

  const handleStopExternal = async (pid: number) => {
    await fetch("/api/tunnels/stop-external", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ pid }),
    });
    onRefresh();
  };

  const copyUrl = (id: string, url: string) => {
    navigator.clipboard.writeText(url);
    setCopiedId(id);
    setTimeout(() => setCopiedId(""), 1800);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <section className="step-section" style={{ marginBottom: 0 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 12 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 18, display: "flex", alignItems: "center", gap: 8 }}>
              <Globe size={20} color="var(--accent)" />
              <span>{t.tunnelsTitle}</span>
            </h2>
            <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--text-muted)" }}>{t.tunnelsSub}</p>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 10 }}>
              <span className={`pill ${tunnels.installed ? "ok" : "warn"}`}>
                <ShieldCheck size={13} />
                {tunnels.installed
                  ? `cloudflared sẵn sàng (${tunnels.version || tunnels.binaryPath})`
                  : vi
                    ? "Chưa tìm thấy cloudflared"
                    : "cloudflared not installed"}
              </span>
              {lanUrls.map((u) => (
                <span key={u} className="pill">
                  <Smartphone size={12} /> LAN: <code>{u}</code>
                </span>
              ))}
            </div>
          </div>

          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              type="button"
              className="btn-primary"
              style={{ width: "auto" }}
              disabled={busy || !tunnels.installed}
              onClick={handleQuickWebTunnel}
            >
              <Globe size={16} />
              <span>{t.quickOpenWebTunnelBtn}</span>
            </button>
            <button type="button" className="btn-secondary" onClick={openCreate}>
              <Plus size={15} />
              <span>{t.createTunnelBtn}</span>
            </button>
            <button type="button" className="btn-secondary" onClick={onRefresh}>
              <RotateCw size={14} />
            </button>
          </div>
        </div>

        {errMsg && (
          <div style={{ marginTop: 12, color: "var(--red-text)", fontSize: 13 }}>
            ⚠ {errMsg}
          </div>
        )}
      </section>

      {showForm && (
        <form className="step-section" style={{ marginBottom: 0 }} onSubmit={handleSaveTunnel}>
          <h3 style={{ marginTop: 0, fontSize: 15.5 }}>
            {editingId
              ? vi
                ? "Cập nhật Cấu hình Cloudflare Tunnel"
                : "Update Cloudflare Tunnel"
              : vi
                ? "Tạo mới Cloudflare Tunnel"
                : "Create New Cloudflare Tunnel"}
          </h3>

          <div className="field-row">
            <div className="field">
              <label>{vi ? "Tên gợi nhớ Tunnel" : "Tunnel Name"}</label>
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} required />
            </div>
            <div className="field">
              <label>{vi ? "Chế độ Tunnel" : "Tunnel Mode"}</label>
              <select className="select" value={mode} onChange={(e) => setMode(e.target.value as any)}>
                <option value="quick">Quick Tunnel (*.trycloudflare.com — Không cần tài khoản)</option>
                <option value="token">Cloudflare Zero Trust Token (--token)</option>
                <option value="named">Named Tunnel (cloudflared tunnel run &lt;name/id&gt;)</option>
              </select>
            </div>
          </div>

          <div className="field-row" style={{ marginTop: 10 }}>
            {mode === "quick" && (
              <div className="field">
                <label>{vi ? "Địa chỉ dịch vụ cục bộ cần mở (URL / Port)" : "Local Service URL"}</label>
                <input
                  className="input"
                  value={targetUrl}
                  onChange={(e) => setTargetUrl(e.target.value)}
                  placeholder="http://127.0.0.1:4320"
                  required
                />
              </div>
            )}
            {mode === "token" && (
              <>
                <div className="field">
                  <label>Cloudflare Tunnel Token {editingId ? "(Để trống nếu giữ nguyên)" : ""}</label>
                  <input
                    className="input"
                    type="password"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    placeholder="eyJhIjoi..."
                  />
                </div>
                <div className="field">
                  <label>{vi ? "Tên miền công khai (Tùy chọn để hiển thị nhanh)" : "Public Hostname (Optional)"}</label>
                  <input
                    className="input"
                    value={publicHostname}
                    onChange={(e) => setPublicHostname(e.target.value)}
                    placeholder="https://studio.tenmiencuaban.com"
                  />
                </div>
              </>
            )}
            {mode === "named" && (
              <>
                <div className="field">
                  <label>{vi ? "Tên hoặc UUID của Tunnel" : "Tunnel Name or UUID"}</label>
                  <input
                    className="input"
                    value={tunnelNameOrId}
                    onChange={(e) => setTunnelNameOrId(e.target.value)}
                    placeholder="doto-studio-tunnel"
                  />
                </div>
                <div className="field">
                  <label>{vi ? "Tên miền công khai" : "Public URL"}</label>
                  <input
                    className="input"
                    value={publicHostname}
                    onChange={(e) => setPublicHostname(e.target.value)}
                    placeholder="https://studio.example.com"
                  />
                </div>
              </>
            )}

            <div className="field">
              <label>{vi ? "Giao thức kết nối (--protocol)" : "Transport Protocol"}</label>
              <select className="select" value={protocol} onChange={(e) => setProtocol(e.target.value as any)}>
                <option value="http2">http2 (Ổn định nhất xuyên tường lửa)</option>
                <option value="auto">auto (Tự động QUIC / HTTP2)</option>
                <option value="quic">quic (Tốc độ UDP thấp trễ)</option>
              </select>
            </div>
          </div>

          <div style={{ marginTop: 12, display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: 10 }}>
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, cursor: "pointer" }}>
              <input type="checkbox" checked={autoStart} onChange={(e) => setAutoStart(e.target.checked)} />
              <span>{vi ? "Tự động bật lại Tunnel này mỗi khi khởi động ứng dụng" : "Auto-start tunnel when app launches"}</span>
            </label>

            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" className="btn-secondary" onClick={() => setShowForm(false)}>
                {vi ? "Huỷ" : "Cancel"}
              </button>
              <button type="submit" className="btn-primary" style={{ width: "auto", minWidth: 160 }} disabled={busy}>
                {editingId ? (vi ? "Lưu & Áp dụng" : "Update Tunnel") : vi ? "Tạo & Chạy Ngay" : "Create & Start"}
              </button>
            </div>
          </div>
        </form>
      )}

      {/* Danh sách Tunnel CRUD */}
      <section className="step-section" style={{ marginBottom: 0 }}>
        <h3 style={{ marginTop: 0, fontSize: 15 }}>{t.managedTunnelsTitle} ({tunnels.managed.length})</h3>
        {tunnels.managed.length === 0 ? (
          <div style={{ color: "var(--text-muted)", fontSize: 13 }}>
            {vi
              ? "Chưa có Cloudflare Tunnel nào. Bấm nút \"Mở nhanh Cloudflare Tunnel cho Web này\" ở trên để nhận ngay địa chỉ HTTPS công khai dùng trên iPad / iPhone / Android!"
              : "No tunnels configured yet. Click 'Open Cloudflare Tunnel for This Web App' above to get an instant public HTTPS URL!"}
          </div>
        ) : (
          <div className="tunnels-grid">
            {tunnels.managed.map((tun) => (
              <div key={tun.id} className={`tunnel-card ${tun.status === "running" ? "running" : ""}`}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 8 }}>
                  <div>
                    <div style={{ fontWeight: 800, fontSize: 15 }}>{tun.name}</div>
                    <div style={{ fontSize: 11.5, color: "var(--text-muted)", fontFamily: "var(--font-mono)", marginTop: 2 }}>
                      {tun.mode.toUpperCase()} · {tun.protocol} {tun.pid ? `· PID ${tun.pid}` : ""}
                    </div>
                  </div>
                  <span
                    className={`dot-badge ${
                      tun.status === "running" ? "ready" : tun.status === "starting" ? "partial" : "missing"
                    }`}
                  >
                    {tun.status === "running"
                      ? vi ? "● Đang mở" : "● Running"
                      : tun.status === "starting"
                        ? vi ? "◌ Đang cấp URL..." : "◌ Connecting..."
                        : tun.status === "error"
                          ? "✗ Error"
                          : vi ? "■ Đã dừng" : "■ Stopped"}
                  </span>
                </div>

                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  {vi ? "Đích cục bộ:" : "Local Target:"} <code>{tun.targetUrl}</code>
                </div>

                {tun.publicUrl ? (
                  <div
                    style={{
                      background: "rgba(16, 185, 129, 0.1)",
                      border: "1px solid rgba(16, 185, 129, 0.35)",
                      borderRadius: 10,
                      padding: "9px 11px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 8,
                      flexWrap: "wrap",
                    }}
                  >
                    <a
                      href={tun.publicUrl}
                      target="_blank"
                      rel="noreferrer"
                      style={{
                        color: "var(--green-text)",
                        fontWeight: 700,
                        fontSize: 12.5,
                        fontFamily: "var(--font-mono)",
                        wordBreak: "break-all",
                      }}
                    >
                      {tun.publicUrl} <ExternalLink size={12} style={{ verticalAlign: "middle" }} />
                    </a>
                    <button type="button" className="btn-secondary" onClick={() => copyUrl(tun.id, tun.publicUrl)}>
                      {copiedId === tun.id ? <Check size={13} /> : <Copy size={13} />}
                      <span>{copiedId === tun.id ? "OK" : t.copyLinkBtn}</span>
                    </button>
                  </div>
                ) : tun.status === "starting" ? (
                  <div style={{ fontSize: 12, color: "var(--blue)" }}>
                    {vi ? "Đang thương lượng chứng chỉ HTTPS với mạng biên Cloudflare (~3–6 giây)..." : "Registering edge HTTPS URL with Cloudflare..."}
                  </div>
                ) : null}

                {tun.lastError ? (
                  <div style={{ fontSize: 12, color: "var(--red-text)" }}>{tun.lastError}</div>
                ) : null}

                <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: "auto", paddingTop: 6 }}>
                  {tun.status === "running" || tun.status === "starting" ? (
                    <>
                      <button type="button" className="btn-secondary btn-danger" onClick={() => handleAction(tun.id, "stop")}>
                        <Square size={13} />
                        <span>{t.stopBtn}</span>
                      </button>
                      <button type="button" className="btn-secondary" onClick={() => handleAction(tun.id, "restart")}>
                        <RotateCw size={13} />
                        <span>{t.restartBtn}</span>
                      </button>
                    </>
                  ) : (
                    <button type="button" className="btn-secondary" onClick={() => handleAction(tun.id, "start")}>
                      <Play size={13} />
                      <span>{t.startBtn}</span>
                    </button>
                  )}

                  <button type="button" className="btn-secondary" onClick={() => openEdit(tun)}>
                    <Edit3 size={13} />
                    <span>{t.editBtn}</span>
                  </button>

                  <button
                    type="button"
                    className="btn-secondary"
                    onClick={() => setExpandedLogsId(expandedLogsId === tun.id ? "" : tun.id)}
                  >
                    <Terminal size={13} />
                    <span>Logs</span>
                  </button>

                  <button
                    type="button"
                    className="btn-secondary btn-danger"
                    style={{ marginLeft: "auto" }}
                    onClick={() => handleAction(tun.id, "delete")}
                  >
                    <Trash2 size={13} />
                    <span>{t.deleteBtn}</span>
                  </button>
                </div>

                {expandedLogsId === tun.id && (
                  <div className="cmd-box" style={{ maxHeight: 160, overflowY: "auto", fontSize: 11 }}>
                    {tun.logs.length === 0
                      ? "No logs yet."
                      : tun.logs.map((l, idx) => <div key={idx}>{l.text}</div>)}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Các tiến trình cloudflared chạy bên ngoài trên thiết bị */}
      <section className="step-section" style={{ marginBottom: 0 }}>
        <h3 style={{ marginTop: 0, fontSize: 15 }}>
          {t.externalTunnelsTitle} ({tunnels.external.length})
        </h3>
        {tunnels.external.length === 0 ? (
          <div style={{ fontSize: 12.5, color: "var(--text-muted)" }}>
            {vi
              ? "Không có tiến trình cloudflared ngoài luồng nào khác trên thiết bị (mọi tunnel đang chạy đều đã nằm trong bảng CRUD ở trên)."
              : "No unmanaged external cloudflared processes detected on this device."}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {tunnels.external.map((ext) => (
              <div
                key={ext.pid}
                className="tunnel-card"
                style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap" }}
              >
                <div style={{ minWidth: 0, flex: 1 }}>
                  <div style={{ fontWeight: 700, fontSize: 13.5 }}>
                    PID {ext.pid} {ext.targetHint ? `· ${ext.targetHint}` : ""}
                  </div>
                  <div style={{ fontSize: 11.5, fontFamily: "var(--font-mono)", color: "var(--text-muted)", wordBreak: "break-all" }}>
                    {ext.commandLine}
                  </div>
                </div>
                <div style={{ display: "flex", gap: 8 }}>
                  <button type="button" className="btn-secondary" onClick={() => handleAdopt(ext.pid)}>
                    <Plus size={13} />
                    <span>{t.adoptTunnelBtn}</span>
                  </button>
                  <button type="button" className="btn-secondary btn-danger" onClick={() => handleStopExternal(ext.pid)}>
                    <Square size={13} />
                    <span>{t.stopBtn}</span>
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
};

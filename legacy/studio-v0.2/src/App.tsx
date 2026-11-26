import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Clapperboard,
  Cpu,
  Gauge,
  Play,
  Square,
  FolderOpen,
  RefreshCw,
  Copy,
  Check,
  Sparkles,
  Settings as SettingsIcon,
  Film,
  Terminal,
  ExternalLink,
  AlertCircle,
  CheckCircle2,
  Sliders,
  Upload,
  Sun,
  Moon,
  Languages,
  Globe,
  BookOpen,
  Image as ImageIcon,
  Camera,
  Layers,
  Trash2,
  Share2,
  Download,
  LayoutGrid,
  List,
  Plus,
} from "lucide-react";
import type {
  BatchSessionDraft,
  BootstrapPayload,
  JobItem,
  LangCode,
  ModelOption,
  ProviderId,
  ProviderInfo,
  SkillItem,
  ThemeMode,
  UploadedMediaItem,
  VideoProjectItem,
} from "./types";
import { UI_TEXT } from "./i18n";
import { SkillImportModal } from "./components/SkillImportModal";
import { TunnelManagerTab } from "./components/TunnelManagerTab";
import { MemoryHandoffTab } from "./components/MemoryHandoffTab";

type TabId = "studio" | "jobs" | "videos" | "tunnels" | "memory" | "settings";
type RunMode = "single" | "same-ai" | "multi-ai" | "custom-queue";

function todaySlug() {
  const d = new Date();
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

export const App: React.FC = () => {
  const [data, setData] = useState<BootstrapPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<TabId>("studio");
  const [errorBanner, setErrorBanner] = useState<string>("");
  const [copied, setCopied] = useState(false);

  // Giao diện Sáng / Tối & Ngôn ngữ Tiếng Việt / Tiếng Anh
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem("doto_theme");
    return saved === "light" ? "light" : "dark";
  });
  const [lang, setLang] = useState<LangCode>(() => {
    const saved = localStorage.getItem("doto_lang");
    return saved === "en" ? "en" : "vi";
  });

  const t = UI_TEXT[lang];
  const vi = lang === "vi";

  // 4 bước chính: Skill -> AI -> Model từng AI -> Effort từng Model
  const [skillId, setSkillId] = useState<string>("stickman-video");
  const [providerId, setProviderId] = useState<ProviderId>("claude");
  const [perProviderModel, setPerProviderModel] = useState<Record<ProviderId, string>>({
    claude: "claude-opus-5-5",
    codex: "gpt-6.2-sol",
    antigravity: "gemini-4-carbon",
  });
  const [perProviderEffort, setPerProviderEffort] = useState<Record<ProviderId, string>>({
    claude: "high",
    codex: "high",
    antigravity: "high",
  });
  const [customModelInput, setCustomModelInput] = useState<string>("");

  // Thông số kịch bản & tùy chỉnh nơi lưu
  const [brief, setBrief] = useState<string>(
    "Làm bản tin hôm nay: chọn 3 tin đáng chú ý nhất trong ngày, mỗi tin đối chiếu ≥ 2 báo chính thống, ưu tiên ảnh/video thật của báo (ghi nguồn trên hình), nhân vật kiểu \"pro\" chỉ diễn phần khuyến cáo/mô phỏng."
  );
  const [topic, setTopic] = useState<string>("thoi-su");
  const [format, setFormat] = useState<string>("9:16");
  const [character, setCharacter] = useState<string>("pro");
  const [stickMode, setStickMode] = useState<string>("ban-tin");
  const [renderStage, setRenderStage] = useState<"draft" | "final">("draft");
  const [projectFolder, setProjectFolder] = useState<string>(`_video/${todaySlug()}-ban-tin`);
  const [videoOutputDir, setVideoOutputDir] = useState<string>("");
  const [useRawPrompt, setUseRawPrompt] = useState<boolean>(false);
  const [rawPrompt, setRawPrompt] = useState<string>("");

  // Đa Phiên Song Song (Cùng 1 AI hoặc Nhiều AI)
  const [runMode, setRunMode] = useState<RunMode>("single");
  const [sameAiCount, setSameAiCount] = useState<number>(2);
  const [sameAiVariantBriefs, setSameAiVariantBriefs] = useState<string>("");
  const [multiAiSelected, setMultiAiSelected] = useState<Record<ProviderId, boolean>>({
    antigravity: true,
    claude: true,
    codex: true,
  });
  const [multiAiSessionsPerProvider, setMultiAiSessionsPerProvider] = useState<number>(1);
  const [customQueue, setCustomQueue] = useState<BatchSessionDraft[]>([]);

  // Tư liệu App Photos (iPad · iPhone · Android · PC)
  const [attachedMedia, setAttachedMedia] = useState<UploadedMediaItem[]>([]);
  const [uploadingMedia, setUploadingMedia] = useState<boolean>(false);
  const photosInputRef = useRef<HTMLInputElement | null>(null);
  const cameraInputRef = useRef<HTMLInputElement | null>(null);

  // Import Skill Modal
  const [importModalOpen, setImportModalOpen] = useState<boolean>(false);

  // Preview CLI & Jobs & Videos
  const [cmdPreview, setCmdPreview] = useState<{ prompt: string; display: string }>({ prompt: "", display: "" });
  const [submitting, setSubmitting] = useState<boolean>(false);
  const [jobs, setJobs] = useState<JobItem[]>([]);
  const [activeJobId, setActiveJobId] = useState<string>("");
  const [jobsViewMode, setJobsViewMode] = useState<"single" | "grid">("single");
  const [jobsProviderFilter, setJobsProviderFilter] = useState<string>("all");
  const [selectedVideoId, setSelectedVideoId] = useState<string>("");
  const [selectedVideoFile, setSelectedVideoFile] = useState<string>("");
  const [showSheet, setShowSheet] = useState<boolean>(false);

  // Settings form
  const [skillRootInput, setSkillRootInput] = useState<string>("");
  const [customSkillSaveDirInput, setCustomSkillSaveDirInput] = useState<string>("");
  const [videoOutputDirInput, setVideoOutputDirInput] = useState<string>("");
  const [mediaUploadDirInput, setMediaUploadDirInput] = useState<string>("");
  const [autoInjectSharedMemory, setAutoInjectSharedMemory] = useState<boolean>(true);
  const [autoLogHandoff, setAutoLogHandoff] = useState<boolean>(true);
  const [syncSkillLinks, setSyncSkillLinks] = useState<boolean>(true);
  const [claudePerm, setClaudePerm] = useState<string>("bypassPermissions");
  const [codexSandbox, setCodexSandbox] = useState<string>("danger-full-access");
  const [agyLsAddress, setAgyLsAddress] = useState<string>("");
  const [agyCsrfToken, setAgyCsrfToken] = useState<string>("");
  const [savingSettings, setSavingSettings] = useState<boolean>(false);

  // Áp dụng Theme Sáng / Tối & Ngôn ngữ lên thẻ <html>
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    document.documentElement.setAttribute("lang", lang);
    localStorage.setItem("doto_theme", theme);
    localStorage.setItem("doto_lang", lang);
    const metaTheme = document.querySelector('meta[name="theme-color"]');
    if (metaTheme) metaTheme.setAttribute("content", theme === "light" ? "#f1f5f9" : "#070a12");
  }, [theme, lang]);

  const toggleTheme = () => {
    const next: ThemeMode = theme === "dark" ? "light" : "dark";
    setTheme(next);
    fetch("/api/settings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ theme: next }),
    }).catch(() => {});
  };

  const toggleLang = () => {
    const next: LangCode = lang === "vi" ? "en" : "vi";
    setLang(next);
    fetch("/api/settings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ language: next }),
    }).catch(() => {});
  };

  const fetchBootstrap = async (refresh = false) => {
    try {
      setLoading(true);
      const res = await fetch(`/api/bootstrap${refresh ? "?refresh=1" : ""}`);
      const json: BootstrapPayload = await res.json();
      setData(json);
      setJobs(json.jobs || []);
      if (json.jobs?.[0] && !activeJobId) setActiveJobId(json.jobs[0].id);
      if (json.videos?.[0] && !selectedVideoId) {
        setSelectedVideoId(json.videos[0].id);
        setSelectedVideoFile(json.videos[0].primaryVideo?.path || "");
      }

      const s = json.settings;
      if (s) {
        if (!localStorage.getItem("doto_theme") && s.theme) setTheme(s.theme);
        if (!localStorage.getItem("doto_lang") && s.language) setLang(s.language);
        setSkillId(s.selectedSkill || "stickman-video");
        setProviderId(s.selectedProvider || "claude");
        setPerProviderModel({
          claude: s.providerPrefs.claude.model || "claude-opus-5-5",
          codex: s.providerPrefs.codex.model || json.providers.find((p) => p.id === "codex")?.models?.[0]?.id || "gpt-6.2-sol",
          antigravity: s.providerPrefs.antigravity.model || "gemini-4-carbon",
        });
        setPerProviderEffort({
          claude: s.providerPrefs.claude.effort || "high",
          codex: s.providerPrefs.codex.effort || "high",
          antigravity: s.providerPrefs.antigravity.effort || "high",
        });
        setSkillRootInput(s.skillRoot || "");
        setCustomSkillSaveDirInput(s.customSkillSaveDir || s.skillRoot || "");
        setVideoOutputDir(s.videoOutputDir || "");
        setVideoOutputDirInput(s.videoOutputDir || "");
        setMediaUploadDirInput(s.mediaUploadDir || "");
        setAutoInjectSharedMemory(s.autoInjectSharedMemory !== false);
        setAutoLogHandoff(s.autoLogHandoff !== false);
        setSyncSkillLinks(s.syncSkillLinks !== false);
        setClaudePerm(s.providerPrefs.claude.permissionMode || "bypassPermissions");
        setCodexSandbox(s.providerPrefs.codex.sandbox || "danger-full-access");
        setAgyLsAddress(s.providerPrefs.antigravity.lsAddress || "");
        setAgyCsrfToken(s.providerPrefs.antigravity.csrfToken || "");
      }
      setErrorBanner("");
    } catch (err: any) {
      setErrorBanner(`Không kết nối được máy chủ cục bộ: ${err?.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchBootstrap(false);
  }, []);

  // SSE thời gian thực cho cả Tiến trình AI (Jobs) & Cloudflare Tunnel
  useEffect(() => {
    const es = new EventSource("/api/events");
    es.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (msg.type === "jobs:cleared" && Array.isArray(msg.jobs)) {
          setJobs(msg.jobs);
          return;
        }
        if (msg.job) {
          setJobs((prev) => {
            const idx = prev.findIndex((x) => x.id === msg.job.id);
            if (idx === -1) return [msg.job, ...prev];
            const copy = [...prev];
            copy[idx] = msg.job;
            return copy;
          });
          if (msg.type === "job:done") {
            fetch("/api/videos")
              .then((r) => r.json())
              .then((vj) => {
                if (vj.videos) setData((d) => (d ? { ...d, videos: vj.videos } : d));
              })
              .catch(() => {});
          }
        }
        if (msg.type?.startsWith("tunnel:")) {
          fetch("/api/tunnels")
            .then((r) => r.json())
            .then((tj) => {
              if (tj.ok) setData((d) => (d ? { ...d, tunnels: tj } : d));
            })
            .catch(() => {});
        }
      } catch {
        // ignore
      }
    };
    return () => es.close();
  }, []);

  const selectedSkill: SkillItem | undefined = useMemo(
    () => data?.skillsInfo.skills.find((s) => s.id === skillId) || data?.skillsInfo.skills[0],
    [data, skillId]
  );

  const selectedProvider: ProviderInfo | undefined = useMemo(
    () => data?.providers.find((p) => p.id === providerId) || data?.providers[0],
    [data, providerId]
  );

  const activeModelId = customModelInput.trim() || perProviderModel[providerId] || "";

  const selectedModelObj: ModelOption | undefined = useMemo(
    () => selectedProvider?.models.find((m) => m.id === perProviderModel[providerId]) || selectedProvider?.models[0],
    [selectedProvider, perProviderModel, providerId]
  );

  useEffect(() => {
    if (!selectedModelObj) return;
    const allowed = selectedModelObj.efforts || [];
    const curEff = perProviderEffort[providerId];
    if (allowed.length === 0) {
      if (curEff !== "") setPerProviderEffort((prev) => ({ ...prev, [providerId]: "" }));
    } else if (!allowed.includes(curEff)) {
      const nextEff =
        selectedModelObj.defaultEffort && allowed.includes(selectedModelObj.defaultEffort)
          ? selectedModelObj.defaultEffort
          : allowed[allowed.length - 1];
      setPerProviderEffort((prev) => ({ ...prev, [providerId]: nextEff }));
    }
  }, [providerId, selectedModelObj]);

  const activeEffort =
    selectedModelObj && selectedModelObj.efforts.length === 0 && !customModelInput.trim()
      ? ""
      : perProviderEffort[providerId] || "";

  useEffect(() => {
    const timer = setTimeout(() => {
      fetch("/api/preview-command", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          skillId,
          providerId,
          model: activeModelId,
          effort: activeEffort,
          brief,
          topic,
          format,
          character: selectedSkill?.supportsCharacter ? character : "",
          stickMode: selectedSkill?.supportsStickMode ? stickMode : "",
          renderStage,
          projectFolder,
          outputDir: videoOutputDir,
          mediaFiles: attachedMedia.map((m) => m.path),
          rawPrompt: useRawPrompt ? rawPrompt : "",
        }),
      })
        .then((r) => r.json())
        .then((res) => {
          if (res.ok) setCmdPreview({ prompt: res.prompt, display: res.display });
        })
        .catch(() => {});
    }, 150);
    return () => clearTimeout(timer);
  }, [
    skillId,
    providerId,
    activeModelId,
    activeEffort,
    brief,
    topic,
    format,
    character,
    stickMode,
    renderStage,
    projectFolder,
    videoOutputDir,
    attachedMedia,
    useRawPrompt,
    rawPrompt,
    selectedSkill,
  ]);

  const pickOsFolder = async (setter: (val: string) => void) => {
    try {
      const res = await fetch("/api/pick-path", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ kind: "folder" }),
      });
      const j = await res.json();
      if (j.ok && j.path) setter(j.path);
    } catch {}
  };

  const handleSelectSkill = (sk: SkillItem) => {
    setSkillId(sk.id);
    setFormat(sk.defaultFormat);
    setTopic(sk.defaultTopic);
    if (sk.quickTemplates?.[0]) {
      const tpl = sk.quickTemplates[0];
      setBrief(tpl.brief);
      if (tpl.character) setCharacter(tpl.character);
      if (tpl.stickMode) setStickMode(tpl.stickMode);
    }
    setProjectFolder(`${todaySlug()}-${sk.id.replace(/-video$/, "")}`);
  };

  const handleDeleteCustomSkill = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (!confirm(vi ? `Xoá Skill mở rộng "${id}"?` : `Delete imported skill "${id}"?`)) return;
    const res = await fetch(`/api/skills/${encodeURIComponent(id)}`, { method: "DELETE" });
    const j = await res.json();
    if (j.ok) {
      if (skillId === id) setSkillId("stickman-video");
      await fetchBootstrap(true);
    }
  };

  // Tải ảnh/video từ App Photos trên iPhone / iPad / Android
  const handlePhotosSelected = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setUploadingMedia(true);
    setErrorBanner("");
    try {
      const newlyAdded: UploadedMediaItem[] = [];
      for (let i = 0; i < files.length; i++) {
        const f = files[i];
        const res = await fetch("/api/media-upload", {
          method: "POST",
          headers: {
            "Content-Type": f.type || "application/octet-stream",
            "X-File-Name": encodeURIComponent(f.name),
          },
          body: f,
        });
        const j = await res.json();
        if (j.ok && j.file) newlyAdded.push(j.file);
      }
      if (newlyAdded.length) {
        setAttachedMedia((prev) => [...prev, ...newlyAdded]);
        await fetchBootstrap(false);
      }
    } catch (err: any) {
      setErrorBanner(err?.message || String(err));
    } finally {
      setUploadingMedia(false);
    }
  };

  // Mở ứng dụng Photos gốc trên iPhone / iPad (photos-redirect://) hoặc Android Gallery
  const handleOpenDevicePhotosApp = () => {
    const ua = navigator.userAgent || "";
    const isIOS = /iPad|iPhone|iPod/i.test(ua) || (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
    const isAndroid = /Android/i.test(ua);
    if (isIOS) {
      window.location.href = "photos-redirect://";
    } else if (isAndroid) {
      window.location.href = "intent://media/internal/images/media#Intent;scheme=content;action=android.intent.action.VIEW;end";
    } else {
      photosInputRef.current?.click();
    }
  };

  // Lưu Video MP4 trực tiếp vào App Photos trên iPhone / iPad / Android qua Web Share API Level 2
  const handleSaveVideoToPhotos = async (filePath: string, fileName: string) => {
    const downloadUrl = `/api/media?path=${encodeURIComponent(filePath)}&download=1`;
    try {
      const navAny = navigator as any;
      if (navAny.share && navAny.canShare) {
        const resp = await fetch(downloadUrl);
        const blob = await resp.blob();
        const fileObj = new File([blob], fileName, { type: "video/mp4" });
        if (navAny.canShare({ files: [fileObj] })) {
          await navAny.share({
            title: fileName,
            files: [fileObj],
          });
          return;
        }
      }
    } catch {
      // Fallback tải về trực tiếp bên dưới
    }
    const a = document.createElement("a");
    a.href = downloadUrl;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    a.remove();
  };

  // Thêm cấu hình hiện tại vào Hàng Đợi Đa Phiên Tùy Chỉnh
  const handleAddCurrentToCustomQueue = () => {
    const item: BatchSessionDraft = {
      uid: `q_${Date.now()}_${Math.random().toString(36).slice(2, 5)}`,
      sessionLabel: `${providerId.toUpperCase()} · ${skillId}`,
      skillId,
      providerId,
      model: activeModelId,
      effort: activeEffort,
      brief,
      topic,
      format,
      character: selectedSkill?.supportsCharacter ? character : "",
      stickMode: selectedSkill?.supportsStickMode ? stickMode : "",
      renderStage,
      projectFolder: `${projectFolder}-q${customQueue.length + 1}`,
    };
    setCustomQueue((prev) => [...prev, item]);
  };

  // Khởi chạy Đơn Phiên hoặc Đa Phiên Song Song (Cùng 1 AI / Nhiều AI / Hàng Đợi Tùy Chỉnh)
  const handleLaunch = async () => {
    setSubmitting(true);
    setErrorBanner("");
    const mediaPaths = attachedMedia.map((m) => m.path);

    try {
      if (runMode === "single") {
        const res = await fetch("/api/jobs", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            skillId,
            providerId,
            model: activeModelId,
            effort: activeEffort,
            brief,
            topic,
            format,
            character: selectedSkill?.supportsCharacter ? character : "",
            stickMode: selectedSkill?.supportsStickMode ? stickMode : "",
            renderStage,
            projectFolder,
            outputDir: videoOutputDir,
            mediaFiles: mediaPaths,
            rawPrompt: useRawPrompt ? rawPrompt : "",
          }),
        });
        const json = await res.json();
        if (!json.ok) {
          setErrorBanner(json.error || "Không thể khởi chạy tác vụ.");
          return;
        }
        setActiveJobId(json.job.id);
        setJobsViewMode("single");
        setTab("jobs");
        return;
      }

      // Xây dựng danh sách phiên chạy đồng thời
      let sessionsPayload: any[] = [];
      if (runMode === "same-ai") {
        const customLines = sameAiVariantBriefs
          .split(/\r?\n/)
          .map((l) => l.trim())
          .filter(Boolean);
        const count = Math.max(2, Math.min(12, sameAiCount));
        for (let i = 0; i < count; i++) {
          sessionsPayload.push({
            sessionLabel: `${providerId.toUpperCase()} #${i + 1}`,
            skillId,
            providerId,
            model: activeModelId,
            effort: activeEffort,
            brief: customLines[i] || brief,
            topic,
            format,
            character: selectedSkill?.supportsCharacter ? character : "",
            stickMode: selectedSkill?.supportsStickMode ? stickMode : "",
            renderStage,
            projectFolder: `${projectFolder}-s${i + 1}`,
            outputDir: videoOutputDir,
            mediaFiles: mediaPaths,
            rawPrompt: useRawPrompt ? rawPrompt : "",
          });
        }
      } else if (runMode === "multi-ai") {
        const chosenProviders = (["antigravity", "claude", "codex"] as ProviderId[]).filter((p) => multiAiSelected[p]);
        if (chosenProviders.length === 0) {
          setErrorBanner(vi ? "Hãy tích chọn ít nhất 1 AI để chạy song song." : "Please select at least 1 AI engine.");
          return;
        }
        const perAi = Math.max(1, Math.min(5, multiAiSessionsPerProvider));
        for (const pId of chosenProviders) {
          for (let k = 0; k < perAi; k++) {
            sessionsPayload.push({
              sessionLabel: perAi > 1 ? `${pId.toUpperCase()} #${k + 1}` : pId.toUpperCase(),
              skillId,
              providerId: pId,
              model: perProviderModel[pId] || "",
              effort: perProviderEffort[pId] || "",
              brief,
              topic,
              format,
              character: selectedSkill?.supportsCharacter ? character : "",
              stickMode: selectedSkill?.supportsStickMode ? stickMode : "",
              renderStage,
              projectFolder: `${projectFolder}-${pId}${perAi > 1 ? `-${k + 1}` : ""}`,
              outputDir: videoOutputDir,
              mediaFiles: mediaPaths,
            });
          }
        }
      } else {
        if (customQueue.length === 0) {
          setErrorBanner(vi ? "Hàng đợi tùy chỉnh đang trống. Hãy bấm '+ Thêm cấu hình hiện tại' trước." : "Custom queue is empty.");
          return;
        }
        sessionsPayload = customQueue.map((q) => ({
          ...q,
          outputDir: videoOutputDir,
          mediaFiles: mediaPaths,
        }));
      }

      const res = await fetch("/api/jobs/batch", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          outputDir: videoOutputDir,
          mediaFiles: mediaPaths,
          sessions: sessionsPayload,
        }),
      });
      const json = await res.json();
      if (!json.ok) {
        setErrorBanner(json.error || "Không thể khởi chạy loạt phiên song song.");
        return;
      }
      if (json.errors?.length) {
        setErrorBanner(
          json.errors.map((e: any) => `[${e.providerId.toUpperCase()}]: ${e.error}`).join(" | ")
        );
      }
      if (json.started?.[0]) setActiveJobId(json.started[0].id);
      setJobsViewMode("grid");
      setTab("jobs");
    } catch (err: any) {
      setErrorBanner(err?.message || String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleStopJob = async (id: string) => {
    await fetch(`/api/jobs/${encodeURIComponent(id)}/stop`, { method: "POST" });
  };

  const handleStopAllJobs = async () => {
    await fetch("/api/jobs/stop-all", { method: "POST" });
  };

  const handleClearFinished = async () => {
    const res = await fetch("/api/jobs/clear-finished", { method: "POST" });
    const j = await res.json();
    if (j.jobs) setJobs(j.jobs);
  };

  const handleRecordJobHandoff = async (jobId: string) => {
    await fetch("/api/memory/record-job", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jobId }),
    });
    alert(vi ? "Đã ghi kết quả phiên này vào HANDOFF.md!" : "Recorded session summary to HANDOFF.md!");
  };

  const handleCopyPromptAndOpenAntigravity = async () => {
    try {
      await navigator.clipboard.writeText(cmdPreview.prompt);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {}
    await fetch("/api/launch-antigravity", { method: "POST" });
  };

  const handleSaveSettings = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingSettings(true);
    try {
      const res = await fetch("/api/settings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          skillRoot: skillRootInput.trim(),
          customSkillSaveDir: customSkillSaveDirInput.trim(),
          videoOutputDir: videoOutputDirInput.trim(),
          mediaUploadDir: mediaUploadDirInput.trim(),
          theme,
          language: lang,
          autoInjectSharedMemory,
          autoLogHandoff,
          syncSkillLinks,
          providerPrefs: {
            claude: { permissionMode: claudePerm },
            codex: { sandbox: codexSandbox },
            antigravity: { lsAddress: agyLsAddress.trim(), csrfToken: agyCsrfToken.trim() },
          },
        }),
      });
      const json = await res.json();
      if (json.ok) await fetchBootstrap(true);
    } finally {
      setSavingSettings(false);
    }
  };

  const runningCount = jobs.filter((j) => j.status === "running").length;
  const filteredJobs = jobs.filter((j) =>
    jobsProviderFilter === "all"
      ? true
      : jobsProviderFilter === "running"
        ? j.status === "running"
        : j.providerId === jobsProviderFilter
  );
  const activeJob = filteredJobs.find((j) => j.id === activeJobId) || filteredJobs[0] || null;
  const selectedVideo: VideoProjectItem | null =
    data?.videos.find((v) => v.id === selectedVideoId) || data?.videos[0] || null;
  const selectedTopicObj = data?.skillsInfo.topics.find((tp) => tp.id === topic);
  const activePublicTunnel = data?.tunnels.managed.find((tun) => tun.status === "running" && tun.publicUrl);

  const plannedBatchCount =
    runMode === "same-ai"
      ? sameAiCount
      : runMode === "multi-ai"
        ? (["antigravity", "claude", "codex"] as ProviderId[]).filter((p) => multiAiSelected[p]).length *
          multiAiSessionsPerProvider
        : customQueue.length;

  if (loading && !data) {
    return (
      <div className="app-shell" style={{ alignItems: "center", justifyContent: "center", minHeight: "80vh" }}>
        <div style={{ textAlign: "center", color: "var(--text-muted)" }}>
          <RefreshCw size={28} style={{ animation: "spin 1s linear infinite" }} />
          <p>{vi ? "Đang khởi tạo Doto Skill Studio..." : "Initializing Doto Skill Studio..."}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell">
      {/* Thanh điều hướng trung tâm (PC & iPad) */}
      <header className="topbar">
        <div className="brand">
          <div className="brand-logo">DT</div>
          <div>
            <div className="brand-title">
              <span>DOTO SKILL STUDIO</span>
              <span className="pill" style={{ fontSize: 10.5 }}>
                v0.2 · PC · iPad · iPhone
              </span>
            </div>
            <div className="brand-sub">{t.brandSub}</div>
          </div>
        </div>

        <nav className="nav-tabs">
          <button className={`nav-tab ${tab === "studio" ? "active" : ""}`} onClick={() => setTab("studio")}>
            <Clapperboard size={15} />
            <span>{t.tabStudio}</span>
          </button>
          <button className={`nav-tab ${tab === "jobs" ? "active" : ""}`} onClick={() => setTab("jobs")}>
            <Terminal size={15} />
            <span>
              {t.tabJobs} {runningCount > 0 ? `(${runningCount} ●)` : `(${jobs.length})`}
            </span>
          </button>
          <button className={`nav-tab ${tab === "videos" ? "active" : ""}`} onClick={() => setTab("videos")}>
            <Film size={15} />
            <span>
              {t.tabVideos} ({data?.videos.length || 0})
            </span>
          </button>
          <button className={`nav-tab ${tab === "tunnels" ? "active" : ""}`} onClick={() => setTab("tunnels")}>
            <Globe size={15} />
            <span>
              {t.tabTunnels} {activePublicTunnel ? "●" : ""}
            </span>
          </button>
          <button className={`nav-tab ${tab === "memory" ? "active" : ""}`} onClick={() => setTab("memory")}>
            <BookOpen size={15} />
            <span>{t.tabMemory}</span>
          </button>
          <button className={`nav-tab ${tab === "settings" ? "active" : ""}`} onClick={() => setTab("settings")}>
            <SettingsIcon size={15} />
            <span>{t.tabSettings}</span>
          </button>
        </nav>

        <div className="status-pills">
          {activePublicTunnel && (
            <button
              type="button"
              className="pill ok"
              style={{ cursor: "pointer" }}
              onClick={() => setTab("tunnels")}
              title={activePublicTunnel.publicUrl}
            >
              <Globe size={12} />
              <span>Tunnel Online</span>
            </button>
          )}
          <span className={`pill ${data?.skillsInfo.rootExists ? "ok" : "warn"}`}>
            {data?.skillsInfo.rootExists
              ? `✓ ${data.skillsInfo.skills.filter((s) => s.existsOnDisk).length} ${t.skillsReady}`
              : `⚠ ${t.checkSkillDir}`}
          </span>

          {/* Nút Đổi Ngôn Ngữ VI / EN */}
          <button type="button" className="toggle-btn" onClick={toggleLang} title="Switch Language (Tiếng Việt / English)">
            <Languages size={14} color="var(--accent)" />
            <span>{lang === "vi" ? "VI" : "EN"}</span>
          </button>

          {/* Nút Đổi Giao Diện Nền Sáng / Tối */}
          <button type="button" className="toggle-btn" onClick={toggleTheme} title="Toggle Light / Dark Mode">
            {theme === "dark" ? <Sun size={14} color="#fbbf24" /> : <Moon size={14} color="#ea580c" />}
            <span>{theme === "dark" ? t.themeLight : t.themeDark}</span>
          </button>

          <button type="button" className="toggle-btn" onClick={() => fetchBootstrap(true)} title="Refresh">
            <RefreshCw size={13} />
          </button>
        </div>
      </header>

      {/* Thanh điều hướng đáy tối ưu riêng cho iPhone & Android */}
      <nav className="mobile-bottom-nav">
        <button className={`mobile-nav-btn ${tab === "studio" ? "active" : ""}`} onClick={() => setTab("studio")}>
          <Clapperboard size={18} />
          <span>{t.tabStudio}</span>
        </button>
        <button className={`mobile-nav-btn ${tab === "jobs" ? "active" : ""}`} onClick={() => setTab("jobs")}>
          <Terminal size={18} />
          <span>
            {t.tabJobs}
            {runningCount > 0 ? `(${runningCount})` : ""}
          </span>
        </button>
        <button className={`mobile-nav-btn ${tab === "videos" ? "active" : ""}`} onClick={() => setTab("videos")}>
          <Film size={18} />
          <span>{t.tabVideos}</span>
        </button>
        <button className={`mobile-nav-btn ${tab === "tunnels" ? "active" : ""}`} onClick={() => setTab("tunnels")}>
          <Globe size={18} />
          <span>Tunnel</span>
        </button>
        <button className={`mobile-nav-btn ${tab === "memory" ? "active" : ""}`} onClick={() => setTab("memory")}>
          <BookOpen size={18} />
          <span>Memory</span>
        </button>
        <button className={`mobile-nav-btn ${tab === "settings" ? "active" : ""}`} onClick={() => setTab("settings")}>
          <SettingsIcon size={18} />
          <span>{t.tabSettings}</span>
        </button>
      </nav>

      {errorBanner ? (
        <div
          style={{
            background: "rgba(239, 68, 68, 0.14)",
            border: "1px solid rgba(239, 68, 68, 0.45)",
            borderRadius: 12,
            padding: "11px 15px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            flexWrap: "wrap",
            gap: 10,
            color: "var(--red-text)",
            fontSize: 13,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <AlertCircle size={17} />
            <span>{errorBanner}</span>
          </div>
          {providerId === "antigravity" && selectedProvider?.hasDesktop ? (
            <button className="btn-secondary" onClick={handleCopyPromptAndOpenAntigravity}>
              <ExternalLink size={14} />
              <span>{vi ? "Chép lệnh & Mở Antigravity Desktop" : "Copy & Open Antigravity Desktop"}</span>
            </button>
          ) : null}
        </div>
      ) : null}

      {/* ================= TAB 1: STUDIO TẠO VIDEO & ĐA PHIÊN ĐA AI ================= */}
      {tab === "studio" && data ? (
        <div className="studio-grid">
          <div>
            {/* BƯỚC 1: CHỌN HOẶC IMPORT SKILL */}
            <section className="step-section">
              <div className="step-header">
                <div className="step-title-wrap">
                  <div className="step-num">1</div>
                  <div>
                    <h2 className="step-title">{t.step1Title}</h2>
                    <div className="step-hint">{t.step1Hint}</div>
                  </div>
                </div>
                <button type="button" className="btn-secondary" onClick={() => setImportModalOpen(true)}>
                  <Upload size={14} color="var(--accent)" />
                  <span>{t.importSkillBtn}</span>
                </button>
              </div>

              <div className="skills-grid">
                {data.skillsInfo.skills.map((sk) => {
                  const isSel = sk.id === skillId;
                  return (
                    <button
                      key={sk.id}
                      type="button"
                      className={`skill-card ${isSel ? "selected" : ""}`}
                      style={{ ["--skill-color" as any]: sk.color }}
                      onClick={() => handleSelectSkill(sk)}
                    >
                      <div className="skill-card-top">
                        <span className="skill-id">/{sk.id}</span>
                        <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
                          <span className="skill-badge" style={{ background: `${sk.color}28`, color: sk.color }}>
                            {vi ? sk.badge : sk.badgeEn || sk.badge}
                          </span>
                          {sk.isCustom && (
                            <span
                              role="button"
                              title={vi ? "Xoá Skill mở rộng" : "Remove custom skill"}
                              onClick={(e) => handleDeleteCustomSkill(e, sk.id)}
                              style={{ padding: 3, color: "var(--red-text)" }}
                            >
                              <Trash2 size={13} />
                            </span>
                          )}
                        </div>
                      </div>
                      <div className="skill-name">{vi ? sk.title : sk.titleEn || sk.title}</div>
                      <div className="skill-desc">{vi ? sk.summary : sk.summaryEn || sk.summary}</div>
                      <div className="skill-meta">
                        <span>{sk.engine}</span>
                        <span>{sk.estimatedTime}</span>
                      </div>
                    </button>
                  );
                })}
              </div>
            </section>

            {/* BƯỚC 2: CHỌN AI ENGINE */}
            <section className="step-section">
              <div className="step-header">
                <div className="step-title-wrap">
                  <div className="step-num">2</div>
                  <div>
                    <h2 className="step-title">{t.step2Title}</h2>
                    <div className="step-hint">{t.step2Hint}</div>
                  </div>
                </div>
              </div>

              <div className="providers-grid">
                {data.providers.map((prov) => {
                  const isSel = prov.id === providerId;
                  return (
                    <button
                      key={prov.id}
                      type="button"
                      className={`provider-card ${isSel ? "selected" : ""}`}
                      style={{ ["--prov-color" as any]: prov.accent }}
                      onClick={() => {
                        setProviderId(prov.id);
                        setCustomModelInput("");
                      }}
                    >
                      <div className="provider-head">
                        <div>
                          <div className="provider-name">{prov.name}</div>
                          <div className="provider-vendor">{prov.vendor}</div>
                        </div>
                        <span
                          className={`dot-badge ${
                            prov.readyToRun ? "ready" : prov.installed ? "partial" : "missing"
                          }`}
                        >
                          {prov.readyToRun
                            ? vi ? "● Sẵn sàng CLI" : "● CLI Ready"
                            : prov.installed
                              ? vi ? "● Có Desktop" : "● Desktop App"
                              : vi ? "○ Chưa cài" : "○ Missing"}
                        </span>
                      </div>
                      <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.45 }}>{prov.bestFor}</div>
                      <div
                        style={{
                          marginTop: "auto",
                          paddingTop: 5,
                          fontSize: 11,
                          fontFamily: "var(--font-mono)",
                          color: "var(--text-dim)",
                        }}
                      >
                        {prov.skillPrefix}
                        {skillId} · Model: {perProviderModel[prov.id]}
                      </div>
                    </button>
                  );
                })}
              </div>
            </section>

            {/* BƯỚC 3: CHỌN MODEL CỦA TỪNG AI */}
            <section className="step-section">
              <div className="step-header">
                <div className="step-title-wrap">
                  <div className="step-num">3</div>
                  <div>
                    <h2 className="step-title">
                      {t.step3Title} {selectedProvider?.name}
                    </h2>
                    <div className="step-hint">{t.step3Hint}</div>
                  </div>
                </div>
                <Cpu size={18} color="var(--text-muted)" />
              </div>

              <div className="models-grid">
                {selectedProvider?.models.map((m) => {
                  const isSel = !customModelInput.trim() && perProviderModel[providerId] === m.id;
                  return (
                    <button
                      key={m.id}
                      type="button"
                      className={`model-card ${isSel ? "selected" : ""}`}
                      onClick={() => {
                        setCustomModelInput("");
                        setPerProviderModel((prev) => ({ ...prev, [providerId]: m.id }));
                        if (m.defaultEffort && m.efforts.includes(m.defaultEffort)) {
                          setPerProviderEffort((prev) => ({ ...prev, [providerId]: m.defaultEffort }));
                        }
                      }}
                    >
                      <div className="model-name">
                        <span>{m.name}</span>
                        {m.badge ? (
                          <span
                            style={{
                              fontSize: 10,
                              padding: "2px 7px",
                              borderRadius: 999,
                              background: "var(--accent-soft)",
                              color: "var(--accent)",
                            }}
                          >
                            {m.badge}
                          </span>
                        ) : null}
                      </div>
                      <div className="model-slug">{m.id}</div>
                      <div className="model-desc">{m.desc}</div>
                      <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: "auto", paddingTop: 4 }}>
                        {m.efforts.length > 0
                          ? `Effort: ${m.efforts.join(" · ")}`
                          : m.fixedEffortLabel || "Auto Effort"}
                      </div>
                    </button>
                  );
                })}
              </div>

              <div style={{ marginTop: 10, display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <input
                  className="input"
                  style={{ maxWidth: 340, fontSize: 12.5, fontFamily: "var(--font-mono)" }}
                  placeholder={vi ? `Hoặc nhập mã model ${selectedProvider?.name} tuỳ chỉnh...` : `Custom ${selectedProvider?.name} model slug...`}
                  value={customModelInput}
                  onChange={(e) => setCustomModelInput(e.target.value)}
                />
                {customModelInput ? (
                  <button className="btn-secondary" type="button" onClick={() => setCustomModelInput("")}>
                    {vi ? "Dùng danh sách chuẩn" : "Reset"}
                  </button>
                ) : null}
              </div>
            </section>

            {/* BƯỚC 4: CHỌN EFFORT CỦA TỪNG MODEL */}
            <section className="step-section" style={{ marginBottom: 0 }}>
              <div className="step-header">
                <div className="step-title-wrap">
                  <div className="step-num">4</div>
                  <div>
                    <h2 className="step-title">
                      {t.step4Title} {customModelInput.trim() || selectedModelObj?.name}
                    </h2>
                    <div className="step-hint">{t.step4Hint}</div>
                  </div>
                </div>
                <Gauge size={18} color="var(--text-muted)" />
              </div>

              {(() => {
                const effortsList = customModelInput.trim()
                  ? ["low", "medium", "high", "xhigh", "max"]
                  : selectedModelObj?.efforts || [];

                if (effortsList.length === 0) {
                  return (
                    <div className="effort-explain">
                      <strong>{selectedModelObj?.fixedEffortLabel || "Auto Reasoning"}:</strong>{" "}
                      <code>{selectedModelObj?.name}</code>
                    </div>
                  );
                }

                const curInfo = data.effortsMeta[activeEffort];
                return (
                  <>
                    <div className="effort-track">
                      {effortsList.map((eff) => {
                        const meta = data.effortsMeta[eff] || { label: eff, vi: eff, desc: "" };
                        const isSel = activeEffort === eff;
                        return (
                          <button
                            key={eff}
                            type="button"
                            className={`effort-pill ${isSel ? "selected" : ""}`}
                            onClick={() => setPerProviderEffort((prev) => ({ ...prev, [providerId]: eff }))}
                          >
                            <div className="effort-code">{meta.label}</div>
                            <div className="effort-vi">{vi ? meta.vi : meta.label}</div>
                          </button>
                        );
                      })}
                    </div>
                    {curInfo ? (
                      <div className="effort-explain">
                        <strong>
                          {curInfo.label} ({curInfo.vi}):
                        </strong>{" "}
                        {curInfo.desc}
                      </div>
                    ) : null}
                  </>
                );
              })()}
            </section>
          </div>

          {/* ================= CỘT PHẢI: TƯ LIỆU APP PHOTOS, TÙY CHỈNH NƠI LƯU & ĐA PHIÊN SONG SONG ================= */}
          <aside className="composer-panel">
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontWeight: 800, fontSize: 15 }}>
                <Sparkles size={18} color="var(--accent)" />
                <span>{t.composerTitle}</span>
              </div>
              <span className="pill" style={{ fontFamily: "var(--font-mono)" }}>
                {selectedProvider?.skillPrefix}
                {skillId}
              </span>
            </div>

            {selectedSkill?.quickTemplates?.length ? (
              <div className="field">
                <label>{t.quickTemplates}</label>
                <div className="template-chips">
                  {selectedSkill.quickTemplates.map((tpl, i) => (
                    <button
                      key={i}
                      type="button"
                      className="chip"
                      onClick={() => {
                        setBrief(tpl.brief);
                        setTopic(tpl.topic);
                        if (tpl.character) setCharacter(tpl.character);
                        if (tpl.stickMode) setStickMode(tpl.stickMode);
                        setUseRawPrompt(false);
                      }}
                    >
                      + {tpl.label}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}

            <div className="field-row">
              <div className="field">
                <label>{t.topicVoice}</label>
                <select className="select" value={topic} onChange={(e) => setTopic(e.target.value)}>
                  <option value="auto">{t.autoTopic}</option>
                  {data.skillsInfo.topics.map((tp) => (
                    <option key={tp.id} value={tp.id}>
                      {tp.label} — {tp.voiceName} (×{tp.voiceSpeed})
                    </option>
                  ))}
                </select>
              </div>

              <div className="field">
                <label>{t.aspectRatio}</label>
                <select className="select" value={format} onChange={(e) => setFormat(e.target.value)}>
                  {(selectedSkill?.formats || ["9:16", "16:9"]).map((f) => (
                    <option key={f} value={f}>
                      {f === "9:16" ? "9:16 (TikTok / Reels / Shorts)" : f === "16:9" ? "16:9 (YouTube)" : "1:1 (Feed)"}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {(selectedSkill?.supportsCharacter || selectedSkill?.supportsStickMode) && (
              <div className="field-row">
                {selectedSkill?.supportsCharacter ? (
                  <div className="field">
                    <label>{t.charStyle}</label>
                    <select className="select" value={character} onChange={(e) => setCharacter(e.target.value)}>
                      <option value="pro">pro — Stickman Pro</option>
                      <option value="stick">stick — Hand-drawn</option>
                      <option value="chibi">chibi — Chibi Anime</option>
                      <option value="pixel">pixel — Pixel 8-bit</option>
                    </select>
                  </div>
                ) : null}

                {selectedSkill?.supportsStickMode ? (
                  <div className="field">
                    <label>{t.stickModeLabel}</label>
                    <select className="select" value={stickMode} onChange={(e) => setStickMode(e.target.value)}>
                      <option value="ban-tin">ban-tin (Daily News)</option>
                      <option value="ho-so-vu-an">ho-so-vu-an (Crime Dossier)</option>
                    </select>
                  </div>
                ) : null}
              </div>
            )}

            {/* TÙY CHỈNH NƠI LƯU VIDEO NGAY TRONG STUDIO */}
            <div className="field">
              <label>{t.saveVideoDir}</label>
              <div className="input-with-btn">
                <input
                  className="input"
                  value={videoOutputDir}
                  onChange={(e) => setVideoOutputDir(e.target.value)}
                  placeholder={`${data.settings.skillRoot}\\_video`}
                />
                <button type="button" className="btn-secondary" onClick={() => pickOsFolder(setVideoOutputDir)}>
                  <FolderOpen size={14} />
                  <span>{t.browseBtn}</span>
                </button>
              </div>
            </div>

            <div className="field-row">
              <div className="field">
                <label>{t.renderStageLabel}</label>
                <select className="select" value={renderStage} onChange={(e) => setRenderStage(e.target.value as "draft" | "final")}>
                  <option value="draft">{t.stageDraft}</option>
                  <option value="final">{t.stageFinal}</option>
                </select>
              </div>
              <div className="field">
                <label>{t.projectSubfolder}</label>
                <input
                  className="input"
                  value={projectFolder}
                  onChange={(e) => setProjectFolder(e.target.value)}
                  placeholder={`${todaySlug()}-ban-tin`}
                />
              </div>
            </div>

            {/* APP PHOTOS TRÊN iPAD / iPHONE / ANDROID */}
            <div className="photos-box">
              <input
                ref={photosInputRef}
                type="file"
                accept="image/*,video/*"
                multiple
                style={{ display: "none" }}
                onChange={(e) => handlePhotosSelected(e.target.files)}
              />
              <input
                ref={cameraInputRef}
                type="file"
                accept="image/*,video/*"
                capture="environment"
                style={{ display: "none" }}
                onChange={(e) => handlePhotosSelected(e.target.files)}
              />

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 6 }}>
                <div style={{ fontSize: 12.5, fontWeight: 700, display: "flex", alignItems: "center", gap: 6 }}>
                  <ImageIcon size={15} color="var(--accent)" />
                  <span>{t.photosBoxTitle}</span>
                </div>
                {uploadingMedia && <span style={{ fontSize: 11.5, color: "var(--accent)" }}>Uploading...</span>}
              </div>
              <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>{t.photosBoxSub}</div>

              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                <button type="button" className="btn-secondary" onClick={() => photosInputRef.current?.click()}>
                  <ImageIcon size={14} color="var(--accent)" />
                  <span>{t.openPhotosAppBtn}</span>
                </button>
                <button type="button" className="btn-secondary" onClick={() => cameraInputRef.current?.click()}>
                  <Camera size={14} />
                  <span>{t.takeCameraBtn}</span>
                </button>
                <button type="button" className="btn-secondary" onClick={handleOpenDevicePhotosApp}>
                  <ExternalLink size={13} />
                  <span>{t.launchPhotosUriBtn}</span>
                </button>
              </div>

              {attachedMedia.length > 0 && (
                <div>
                  <div style={{ fontSize: 11, color: "var(--green-text)", marginBottom: 4, fontWeight: 600 }}>
                    ✓ {t.attachedMediaCount}: {attachedMedia.length}
                  </div>
                  <div className="media-thumbs">
                    {attachedMedia.map((m) => (
                      <div key={m.id} className="media-thumb" title={m.originalName}>
                        {m.kind === "video" ? (
                          <video src={`/api/media?path=${encodeURIComponent(m.path)}`} muted playsInline />
                        ) : (
                          <img src={`/api/media?path=${encodeURIComponent(m.path)}`} alt={m.name} />
                        )}
                        <button
                          type="button"
                          className="media-thumb-remove"
                          onClick={() => setAttachedMedia((prev) => prev.filter((x) => x.id !== m.id))}
                        >
                          ×
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {selectedTopicObj?.voiceWhy ? (
              <div style={{ fontSize: 11.5, color: "var(--text-muted)", background: "var(--bg-chip)", padding: "7px 10px", borderRadius: 9 }}>
                🎙️ <strong>{selectedTopicObj.voiceName}:</strong> {selectedTopicObj.voiceWhy}
              </div>
            ) : null}

            <div className="field">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <label>{useRawPrompt ? t.promptRaw : t.promptGuided}</label>
                <button
                  type="button"
                  className="chip"
                  onClick={() => {
                    if (!useRawPrompt) setRawPrompt(cmdPreview.prompt);
                    setUseRawPrompt(!useRawPrompt);
                  }}
                >
                  <Sliders size={11} />
                  <span>{useRawPrompt ? t.switchToGuided : t.switchToRaw}</span>
                </button>
              </div>
              {useRawPrompt ? (
                <textarea className="textarea" value={rawPrompt} onChange={(e) => setRawPrompt(e.target.value)} />
              ) : (
                <textarea className="textarea" value={brief} onChange={(e) => setBrief(e.target.value)} />
              )}
            </div>

            {/* CHẾ ĐỘ CHẠY ĐA PHIÊN SONG SONG (CÙNG 1 AI HOẶC NHIỀU AI) */}
            <div className="multi-box">
              <div style={{ fontSize: 12.5, fontWeight: 800, display: "flex", alignItems: "center", gap: 6 }}>
                <Layers size={15} color="var(--accent)" />
                <span>{t.multiSessionTitle}</span>
              </div>

              <div className="mode-pills">
                <button
                  type="button"
                  className={`mode-pill ${runMode === "single" ? "active" : ""}`}
                  onClick={() => setRunMode("single")}
                >
                  {t.modeSingle}
                </button>
                <button
                  type="button"
                  className={`mode-pill ${runMode === "same-ai" ? "active" : ""}`}
                  onClick={() => setRunMode("same-ai")}
                >
                  {t.modeSameAiMulti}
                </button>
                <button
                  type="button"
                  className={`mode-pill ${runMode === "multi-ai" ? "active" : ""}`}
                  onClick={() => setRunMode("multi-ai")}
                >
                  {t.modeMultiAi}
                </button>
                <button
                  type="button"
                  className={`mode-pill ${runMode === "custom-queue" ? "active" : ""}`}
                  onClick={() => setRunMode("custom-queue")}
                >
                  {t.modeCustomQueue} ({customQueue.length})
                </button>
              </div>

              {runMode === "same-ai" && (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
                    <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
                      {t.sessionCountLabel} <strong>{selectedProvider?.name}</strong>
                    </span>
                    <input
                      type="number"
                      min={2}
                      max={10}
                      className="input"
                      style={{ width: 82 }}
                      value={sameAiCount}
                      onChange={(e) => setSameAiCount(Math.max(2, Math.min(10, Number(e.target.value) || 2)))}
                    />
                  </div>
                  <textarea
                    className="textarea"
                    style={{ minHeight: 68, fontSize: 12 }}
                    value={sameAiVariantBriefs}
                    onChange={(e) => setSameAiVariantBriefs(e.target.value)}
                    placeholder={
                      vi
                        ? "Tùy chọn: Nhập mỗi dòng 1 chủ đề/link riêng cho từng phiên (để trống nếu muốn chạy N phương án song song cho cùng chủ đề trên)..."
                        : "Optional: 1 custom brief per line for each parallel session..."
                    }
                  />
                </div>
              )}

              {runMode === "multi-ai" && (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{t.selectAisToRun}</div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
                    {(["antigravity", "claude", "codex"] as ProviderId[]).map((pId) => {
                      const pObj = data.providers.find((x) => x.id === pId);
                      return (
                        <label
                          key={pId}
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 6,
                            fontSize: 12.5,
                            fontWeight: 700,
                            padding: "5px 10px",
                            borderRadius: 8,
                            border: "1px solid var(--border)",
                            cursor: "pointer",
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={multiAiSelected[pId]}
                            onChange={(e) => setMultiAiSelected((prev) => ({ ...prev, [pId]: e.target.checked }))}
                          />
                          <span>
                            {pObj?.name || pId} ({perProviderModel[pId]})
                          </span>
                        </label>
                      );
                    })}
                  </div>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: 12 }}>
                    <span style={{ color: "var(--text-muted)" }}>
                      {vi ? "Số phiên nhân bản trên mỗi AI đã chọn:" : "Sessions per selected AI:"}
                    </span>
                    <input
                      type="number"
                      min={1}
                      max={5}
                      className="input"
                      style={{ width: 76 }}
                      value={multiAiSessionsPerProvider}
                      onChange={(e) => setMultiAiSessionsPerProvider(Math.max(1, Math.min(5, Number(e.target.value) || 1)))}
                    />
                  </div>
                </div>
              )}

              {runMode === "custom-queue" && (
                <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
                  <button type="button" className="btn-secondary" onClick={handleAddCurrentToCustomQueue}>
                    <Plus size={14} color="var(--accent)" />
                    <span>{t.addSessionToQueue}</span>
                  </button>
                  {customQueue.map((q, idx) => (
                    <div
                      key={q.uid}
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        gap: 8,
                        padding: "6px 9px",
                        borderRadius: 8,
                        background: "var(--bg-input)",
                        fontSize: 11.5,
                      }}
                    >
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <strong>
                          #{idx + 1} [{q.providerId.toUpperCase()}]
                        </strong>{" "}
                        {q.skillId} · <code>{q.model}</code>
                      </div>
                      <button
                        type="button"
                        className="chip"
                        onClick={() => setCustomQueue((prev) => prev.filter((x) => x.uid !== q.uid))}
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="field">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <label>{t.cliPreviewLabel}</label>
                <button
                  type="button"
                  className="chip"
                  onClick={() => {
                    navigator.clipboard.writeText(cmdPreview.prompt);
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1800);
                  }}
                >
                  {copied ? <Check size={12} /> : <Copy size={12} />}
                  <span>{copied ? t.copiedPrompt : t.copyPrompt}</span>
                </button>
              </div>
              <div className="cmd-box">{cmdPreview.display || "..."}</div>
            </div>

            <button type="button" className="btn-primary" disabled={submitting} onClick={handleLaunch}>
              <Play size={18} fill="currentColor" />
              <span>
                {submitting
                  ? vi ? "Đang khởi chạy..." : "Launching..."
                  : runMode === "single"
                    ? `${t.startSingleBtn} ${selectedProvider?.name} (${activeModelId})`
                    : `${t.startBatchBtn} (${plannedBatchCount} ${t.sessionsUnit})`}
              </span>
            </button>

            {providerId === "antigravity" && !selectedProvider?.hasCli && selectedProvider?.hasDesktop ? (
              <button type="button" className="btn-secondary" onClick={handleCopyPromptAndOpenAntigravity}>
                <ExternalLink size={15} />
                <span>{vi ? "Chép câu lệnh & Mở ngay Antigravity Desktop" : "Copy & Open in Antigravity Desktop"}</span>
              </button>
            ) : null}
          </aside>
        </div>
      ) : null}

      {/* ================= TAB 2: GIÁM SÁT ĐA PHIÊN AI THỜI GIAN THỰC ================= */}
      {tab === "jobs" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          <div
            className="step-section"
            style={{
              marginBottom: 0,
              padding: "12px 16px",
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              flexWrap: "wrap",
              gap: 10,
            }}
          >
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
              {(["all", "running", "antigravity", "claude", "codex"] as const).map((flt) => (
                <button
                  key={flt}
                  type="button"
                  className={`chip ${jobsProviderFilter === flt ? "active" : ""}`}
                  onClick={() => setJobsProviderFilter(flt)}
                >
                  {flt === "all"
                    ? vi ? `Tất cả (${jobs.length})` : `All (${jobs.length})`
                    : flt === "running"
                      ? `● ${t.runningNow} (${runningCount})`
                      : flt.toUpperCase()}
                </button>
              ))}
            </div>

            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <button
                type="button"
                className={`btn-secondary ${jobsViewMode === "single" ? "active" : ""}`}
                onClick={() => setJobsViewMode("single")}
              >
                <List size={14} />
                <span>{t.viewSingleMode}</span>
              </button>
              <button
                type="button"
                className={`btn-secondary ${jobsViewMode === "grid" ? "active" : ""}`}
                onClick={() => setJobsViewMode("grid")}
              >
                <LayoutGrid size={14} />
                <span>{t.viewGridMode}</span>
              </button>
              {runningCount > 0 && (
                <button type="button" className="btn-secondary btn-danger" onClick={handleStopAllJobs}>
                  <Square size={13} />
                  <span>{t.stopAllBtn}</span>
                </button>
              )}
              {jobs.some((j) => j.status !== "running") && (
                <button type="button" className="btn-secondary" onClick={handleClearFinished}>
                  <Trash2 size={13} />
                  <span>{t.clearDoneBtn}</span>
                </button>
              )}
            </div>
          </div>

          {jobsViewMode === "grid" ? (
            <div className="jobs-multi-grid">
              {filteredJobs.map((j) => (
                <div key={j.id} className="step-section" style={{ marginBottom: 0, display: "flex", flexDirection: "column", gap: 10 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
                    <div>
                      <span style={{ fontSize: 12, fontWeight: 800, color: "var(--accent)", textTransform: "uppercase" }}>
                        {j.sessionLabel || j.providerId} · {j.model || "default"}
                      </span>
                      <div style={{ fontWeight: 700, fontSize: 14 }}>{j.skillTitle}</div>
                    </div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <span className={`dot-badge ${j.status === "running" ? "partial" : j.status === "completed" ? "ready" : "missing"}`}>
                        {j.status}
                      </span>
                      {j.status === "running" && (
                        <button type="button" className="btn-secondary btn-danger" onClick={() => handleStopJob(j.id)}>
                          <Square size={12} />
                        </button>
                      )}
                    </div>
                  </div>
                  {j.projectFolder && (
                    <div style={{ fontSize: 11, fontFamily: "var(--font-mono)", color: "var(--text-dim)" }}>
                      📁 {j.projectFolder}
                    </div>
                  )}
                  <div className="log-stream" style={{ maxHeight: 260 }}>
                    {j.logs.slice(-25).map((lg, idx) => (
                      <div key={idx} className={`log-row ${lg.kind}`}>
                        <div>{lg.message}</div>
                        {lg.detail ? <div style={{ fontSize: 11, opacity: 0.78 }}>{lg.detail}</div> : null}
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="jobs-layout">
              <div className="step-section" style={{ marginBottom: 0 }}>
                <h3 style={{ marginTop: 0, fontSize: 14.5 }}>{t.jobsListTitle}</h3>
                {filteredJobs.map((j) => (
                  <button
                    key={j.id}
                    type="button"
                    className={`job-item ${activeJob?.id === j.id ? "active" : ""}`}
                    onClick={() => setActiveJobId(j.id)}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11.5, marginBottom: 4 }}>
                      <span style={{ fontWeight: 700, textTransform: "uppercase", color: "var(--accent)" }}>
                        {j.sessionLabel || j.providerId} · {j.model || "default"}
                      </span>
                      <span
                        style={{
                          color:
                            j.status === "running"
                              ? "var(--blue)"
                              : j.status === "completed"
                                ? "var(--green-text)"
                                : "var(--red-text)",
                          fontWeight: 700,
                        }}
                      >
                        {j.status === "running" ? "● Running" : j.status === "completed" ? "✓ Done" : j.status}
                      </span>
                    </div>
                    <div style={{ fontWeight: 700, fontSize: 13.5 }}>{j.skillTitle}</div>
                    <div
                      style={{
                        fontSize: 11.5,
                        color: "var(--text-muted)",
                        marginTop: 4,
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                    >
                      {j.prompt}
                    </div>
                  </button>
                ))}
              </div>

              <div className="step-section" style={{ marginBottom: 0 }}>
                {!activeJob ? (
                  <div style={{ color: "var(--text-muted)", padding: 20 }}>
                    {vi ? "Chưa có tiến trình nào được chọn." : "Select a session on the left to inspect live logs."}
                  </div>
                ) : (
                  <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: 10 }}>
                      <div>
                        <h3 style={{ margin: 0, fontSize: 16 }}>
                          {activeJob.skillTitle} — {activeJob.providerId.toUpperCase()} ({activeJob.model || "default"}
                          {activeJob.effort ? ` · ${activeJob.effort}` : ""})
                        </h3>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>
                          {new Date(activeJob.createdAt).toLocaleString()}
                          {activeJob.projectFolder ? ` · 📁 ${activeJob.projectFolder}` : ""}
                        </div>
                      </div>
                      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                        {activeJob.status === "running" ? (
                          <button className="btn-secondary btn-danger" onClick={() => handleStopJob(activeJob.id)}>
                            <Square size={14} />
                            <span>{t.stopJobBtn}</span>
                          </button>
                        ) : (
                          <button className="btn-secondary" onClick={() => handleRecordJobHandoff(activeJob.id)}>
                            <BookOpen size={14} />
                            <span>{t.recordHandoffBtn}</span>
                          </button>
                        )}
                        <button
                          className="btn-secondary"
                          onClick={() =>
                            fetch("/api/open-path", {
                              method: "POST",
                              headers: { "Content-Type": "application/json" },
                              body: JSON.stringify({ path: activeJob.outputDir || activeJob.skillRoot }),
                            })
                          }
                        >
                          <FolderOpen size={14} />
                          <span>{t.openSkillDirBtn}</span>
                        </button>
                      </div>
                    </div>

                    <div className="cmd-box">{activeJob.commandDisplay}</div>

                    {activeJob.error ? (
                      <div
                        style={{
                          background: "rgba(239,68,68,0.14)",
                          border: "1px solid rgba(239,68,68,0.4)",
                          padding: "10px 14px",
                          borderRadius: 10,
                          color: "var(--red-text)",
                          fontSize: 13,
                        }}
                      >
                        <strong>Error:</strong> {activeJob.error}
                      </div>
                    ) : null}

                    <div className="log-stream">
                      {activeJob.logs.map((lg, idx) => (
                        <div key={idx} className={`log-row ${lg.kind}`}>
                          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 10.5, opacity: 0.68, marginBottom: 2 }}>
                            <span>[{lg.kind.toUpperCase()}]</span>
                            <span>{new Date(lg.at).toLocaleTimeString()}</span>
                          </div>
                          <div>{lg.message}</div>
                          {lg.detail ? <div style={{ fontSize: 11.5, opacity: 0.8, marginTop: 2 }}>{lg.detail}</div> : null}
                        </div>
                      ))}
                    </div>

                    {activeJob.resultText ? (
                      <div
                        style={{
                          background: "rgba(16,185,129,0.1)",
                          border: "1px solid rgba(16,185,129,0.32)",
                          borderRadius: 12,
                          padding: 14,
                          fontSize: 13.5,
                          lineHeight: 1.55,
                          whiteSpace: "pre-wrap",
                        }}
                      >
                        <div style={{ fontWeight: 700, color: "var(--green-text)", marginBottom: 6, display: "flex", alignItems: "center", gap: 6 }}>
                          <CheckCircle2 size={16} />
                          <span>AI Output Summary:</span>
                        </div>
                        {activeJob.resultText}
                      </div>
                    ) : null}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      ) : null}

      {/* ================= TAB 3: THƯ VIỆN VIDEO & LƯU VỀ APP PHOTOS ================= */}
      {tab === "videos" && data ? (
        <div className="videos-grid">
          <div className="step-section" style={{ marginBottom: 0 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
              <h3 style={{ margin: 0, fontSize: 15 }}>{t.videosListTitle}</h3>
              <button className="btn-secondary" onClick={() => fetchBootstrap(true)}>
                <RefreshCw size={13} />
              </button>
            </div>
            {data.videos.map((v) => (
              <button
                key={v.id}
                type="button"
                className={`job-item ${selectedVideo?.id === v.id ? "active" : ""}`}
                onClick={() => {
                  setSelectedVideoId(v.id);
                  setSelectedVideoFile(v.primaryVideo?.path || "");
                  setShowSheet(false);
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11.5, color: "var(--text-muted)" }}>
                  <span>
                    {v.kind === "video" ? "🎬 Video" : "🧪 Demo"} · {v.format}
                  </span>
                  <span>{v.duration ? `${v.duration.toFixed(1)}s` : ""}</span>
                </div>
                <div style={{ fontWeight: 700, fontSize: 13.5, marginTop: 4 }}>{v.title}</div>
                <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 4, fontFamily: "var(--font-mono)" }}>
                  {v.slug} · {v.videos.length} MP4
                </div>
              </button>
            ))}
          </div>

          <div className="video-player-stage">
            {!selectedVideo ? (
              <div style={{ color: "var(--text-muted)" }}>
                {vi ? "Chưa có video nào trong thư mục lưu." : "No rendered videos found yet."}
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 10 }}>
                  <div>
                    <h3 style={{ margin: 0, fontSize: 17 }}>{selectedVideo.title}</h3>
                    <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                      <code>{selectedVideo.projectDir}</code>
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    {(selectedVideoFile || selectedVideo.primaryVideo) && (
                      <>
                        <button
                          type="button"
                          className="btn-primary"
                          style={{ width: "auto", minHeight: 36, padding: "7px 14px", fontSize: 12.5 }}
                          onClick={() =>
                            handleSaveVideoToPhotos(
                              selectedVideoFile || selectedVideo.primaryVideo!.path,
                              selectedVideo.primaryVideo?.name || `${selectedVideo.slug}.mp4`
                            )
                          }
                        >
                          <Share2 size={14} />
                          <span>{t.saveToPhotosBtn}</span>
                        </button>
                        <a
                          className="btn-secondary"
                          href={`/api/media?path=${encodeURIComponent(selectedVideoFile || selectedVideo.primaryVideo!.path)}&download=1`}
                          download
                        >
                          <Download size={14} />
                          <span>{t.downloadMp4Btn}</span>
                        </a>
                      </>
                    )}
                    {selectedVideo.sheet ? (
                      <button className="btn-secondary" onClick={() => setShowSheet(!showSheet)}>
                        <Film size={14} />
                        <span>{showSheet ? t.viewMp4Btn : t.viewSheetBtn}</span>
                      </button>
                    ) : null}
                    <button
                      className="btn-secondary"
                      onClick={() =>
                        fetch("/api/open-path", {
                          method: "POST",
                          headers: { "Content-Type": "application/json" },
                          body: JSON.stringify({ path: selectedVideoFile || selectedVideo.projectDir }),
                        })
                      }
                    >
                      <FolderOpen size={14} />
                      <span>{t.openSkillDirBtn}</span>
                    </button>
                  </div>
                </div>

                {selectedVideo.videos.length > 1 && !showSheet ? (
                  <div className="template-chips">
                    {selectedVideo.videos.map((vf) => (
                      <button
                        key={vf.path}
                        type="button"
                        className={`chip ${(selectedVideoFile || selectedVideo.primaryVideo?.path) === vf.path ? "active" : ""}`}
                        onClick={() => setSelectedVideoFile(vf.path)}
                      >
                        {vf.name} ({(vf.size / 1e6).toFixed(1)} MB)
                      </button>
                    ))}
                  </div>
                ) : null}

                {showSheet && selectedVideo.sheet ? (
                  <img
                    src={`/api/media?path=${encodeURIComponent(selectedVideo.sheet.path)}`}
                    alt={selectedVideo.sheet.name}
                    style={{ width: "100%", borderRadius: 12, border: "1px solid var(--border)" }}
                  />
                ) : selectedVideoFile || selectedVideo.primaryVideo ? (
                  <video
                    key={selectedVideoFile || selectedVideo.primaryVideo?.path}
                    className="video-el"
                    controls
                    playsInline
                    src={`/api/media?path=${encodeURIComponent(selectedVideoFile || selectedVideo.primaryVideo!.path)}`}
                  />
                ) : (
                  <div style={{ padding: 32, textAlign: "center", color: "var(--text-muted)" }}>
                    {vi ? "Dự án này chưa có file MP4 trong renders/." : "No MP4 file rendered yet."}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      ) : null}

      {/* ================= TAB 4: QUẢN LÝ CRUD CLOUDFLARE TUNNEL ================= */}
      {tab === "tunnels" && data ? (
        <TunnelManagerTab
          lang={lang}
          tunnels={data.tunnels}
          lanUrls={data.lanUrls || []}
          onRefresh={() => fetchBootstrap(false)}
        />
      ) : null}

      {/* ================= TAB 5: BỘ NHỚ CHUNG & BÀN GIAO CHO 3 AI ================= */}
      {tab === "memory" && data ? (
        <MemoryHandoffTab
          lang={lang}
          skillRoot={data.settings.skillRoot}
          appRoot={data.appRoot}
          initialStatus={data.memoryStatus}
          onSynced={() => fetchBootstrap(false)}
        />
      ) : null}

      {/* ================= TAB 6: CẤU HÌNH ĐƯỜNG DẪN LƯU & HỆ THỐNG ================= */}
      {tab === "settings" ? (
        <form className="step-section" onSubmit={handleSaveSettings} style={{ maxWidth: 880 }}>
          <h2 style={{ marginTop: 0, fontSize: 18 }}>{t.settingsTitle}</h2>

          <div style={{ display: "flex", flexDirection: "column", gap: 14, marginTop: 14 }}>
            <div className="field">
              <label>{vi ? "1. Thư mục gốc Bộ Skill Dựng Video (skillRoot)" : "1. Core Skill Suite Root Folder (skillRoot)"}</label>
              <div className="input-with-btn">
                <input className="input" value={skillRootInput} onChange={(e) => setSkillRootInput(e.target.value)} />
                <button type="button" className="btn-secondary" onClick={() => pickOsFolder(setSkillRootInput)}>
                  <FolderOpen size={14} />
                  <span>{t.browseBtn}</span>
                </button>
              </div>
            </div>

            <div className="field">
              <label>{vi ? "2. Nơi lưu mặc định khi Import Skill mới (.zip / .rar / Thư mục)" : "2. Default Save Folder for Imported Skills"}</label>
              <div className="input-with-btn">
                <input
                  className="input"
                  value={customSkillSaveDirInput}
                  onChange={(e) => setCustomSkillSaveDirInput(e.target.value)}
                />
                <button type="button" className="btn-secondary" onClick={() => pickOsFolder(setCustomSkillSaveDirInput)}>
                  <FolderOpen size={14} />
                  <span>{t.browseBtn}</span>
                </button>
              </div>
            </div>

            <div className="field-row">
              <div className="field">
                <label>{vi ? "3. Nơi lưu các Dự án & Video xuất ra (videoOutputDir)" : "3. Custom Rendered Videos Save Folder"}</label>
                <div className="input-with-btn">
                  <input className="input" value={videoOutputDirInput} onChange={(e) => setVideoOutputDirInput(e.target.value)} />
                  <button type="button" className="btn-secondary" onClick={() => pickOsFolder(setVideoOutputDirInput)}>
                    <FolderOpen size={14} />
                  </button>
                </div>
              </div>

              <div className="field">
                <label>{vi ? "4. Nơi lưu Ảnh/Video tải lên từ App Photos điện thoại" : "4. Mobile App Photos Upload Folder"}</label>
                <div className="input-with-btn">
                  <input className="input" value={mediaUploadDirInput} onChange={(e) => setMediaUploadDirInput(e.target.value)} />
                  <button type="button" className="btn-secondary" onClick={() => pickOsFolder(setMediaUploadDirInput)}>
                    <FolderOpen size={14} />
                  </button>
                </div>
              </div>
            </div>

            <div className="field-row">
              <div className="field">
                <label>Claude Code (--permission-mode)</label>
                <select className="select" value={claudePerm} onChange={(e) => setClaudePerm(e.target.value)}>
                  <option value="bypassPermissions">bypassPermissions</option>
                  <option value="auto">auto</option>
                  <option value="acceptEdits">acceptEdits</option>
                </select>
              </div>

              <div className="field">
                <label>OpenAI Codex (--sandbox)</label>
                <select className="select" value={codexSandbox} onChange={(e) => setCodexSandbox(e.target.value)}>
                  <option value="danger-full-access">danger-full-access</option>
                  <option value="workspace-write">workspace-write</option>
                </select>
              </div>
            </div>

            <div className="field-row">
              <div className="field">
                <label>Antigravity Language Server Address</label>
                <input
                  className="input"
                  value={agyLsAddress}
                  onChange={(e) => setAgyLsAddress(e.target.value)}
                  placeholder="localhost:64239"
                />
              </div>
              <div className="field">
                <label>Antigravity CSRF Token</label>
                <input
                  className="input"
                  type="password"
                  value={agyCsrfToken}
                  onChange={(e) => setAgyCsrfToken(e.target.value)}
                  placeholder="x-antigravity-csrf-token"
                />
              </div>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 8, fontSize: 13 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={autoInjectSharedMemory}
                  onChange={(e) => setAutoInjectSharedMemory(e.target.checked)}
                />
                <span>
                  {vi
                    ? "Tự động gắn ngữ cảnh MEMORY.md & HANDOFF.md vào mọi phiên chạy Antigravity / Claude / Codex"
                    : "Auto-inject shared MEMORY.md & HANDOFF.md instructions into every AI session"}
                </span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={autoLogHandoff}
                  onChange={(e) => setAutoLogHandoff(e.target.checked)}
                />
                <span>
                  {vi
                    ? "Tự động ghi nhật ký phiên hoàn tất vào HANDOFF.md để các AI sau nối tiếp"
                    : "Automatically append completed session summaries to HANDOFF.md"}
                </span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={syncSkillLinks}
                  onChange={(e) => setSyncSkillLinks(e.target.checked)}
                />
                <span>
                  {vi
                    ? "Tự động gắn junction Skill mới vào ~/.claude/skills, ~/.codex/skills & ~/.gemini/skills"
                    : "Auto-link imported skills to Claude, Codex & Antigravity skill directories"}
                </span>
              </label>
            </div>

            <div>
              <button type="submit" className="btn-primary" style={{ maxWidth: 240 }} disabled={savingSettings}>
                {savingSettings ? t.savingText : t.saveSettingsBtn}
              </button>
            </div>
          </div>
        </form>
      ) : null}

      <SkillImportModal
        open={importModalOpen}
        lang={lang}
        defaultTargetDir={data?.settings.customSkillSaveDir || data?.settings.skillRoot || ""}
        skillRoot={data?.settings.skillRoot || ""}
        defaultSyncLinks={syncSkillLinks}
        onClose={() => setImportModalOpen(false)}
        onImported={() => fetchBootstrap(true)}
      />
    </div>
  );
};

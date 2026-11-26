export type ProviderId = "claude" | "codex" | "antigravity";
export type ThemeMode = "dark" | "light";
export type LangCode = "vi" | "en";

export interface EffortMeta {
  label: string;
  vi: string;
  desc: string;
}

export interface ModelOption {
  id: string;
  name: string;
  badge?: string;
  defaultEffort: string;
  efforts: string[];
  fixedEffortLabel?: string;
  slugPattern?: string;
  desc: string;
}

export interface ProviderInfo {
  id: ProviderId;
  name: string;
  vendor: string;
  accent: string;
  skillPrefix: string;
  installed: boolean;
  readyToRun: boolean;
  hasCli?: boolean;
  hasDesktop?: boolean;
  hasLanguageServer?: boolean;
  version: string;
  binaryPath: string;
  bestFor: string;
  installHint: string;
  models: ModelOption[];
}

export interface QuickTemplate {
  label: string;
  topic: string;
  character?: string;
  stickMode?: string;
  brief: string;
}

export interface SkillItem {
  id: string;
  title: string;
  titleEn?: string;
  badge: string;
  badgeEn?: string;
  engine: string;
  color: string;
  defaultFormat: string;
  formats: string[];
  defaultTopic: string;
  supportsCharacter: boolean;
  supportsStickMode: boolean;
  estimatedTime: string;
  recommendedProvider: ProviderId;
  summary: string;
  summaryEn?: string;
  rawDescription: string;
  isCustom?: boolean;
  existsOnDisk: boolean;
  skillDir: string;
  skillMdPath: string;
  quickTemplates: QuickTemplate[];
}

export interface TopicProfile {
  id: string;
  label: string;
  voiceName: string;
  voiceSpeed: number;
  voiceWhy?: string;
  theme: string;
  tone?: string;
}

export interface VideoFileEntry {
  name: string;
  path: string;
  size: number;
  mtimeMs: number;
}

export interface VideoProjectItem {
  id: string;
  slug: string;
  kind: "video" | "demo";
  title: string;
  topic: string;
  format: string;
  duration: number;
  projectDir: string;
  mtimeMs: number;
  primaryVideo: VideoFileEntry | null;
  videos: VideoFileEntry[];
  sheet: { name: string; path: string; mtimeMs: number } | null;
  hasFacts: boolean;
  hasReview: boolean;
}

export interface UploadedMediaItem {
  id: string;
  name: string;
  originalName: string;
  path: string;
  size: number;
  kind: "image" | "video" | "file";
  createdAt: number;
}

export interface JobLogEntry {
  at: number;
  kind: "system" | "tool" | "assistant" | "stderr" | "stdout" | "error" | "result";
  message: string;
  detail?: string;
}

export interface JobItem {
  id: string;
  batchId?: string;
  sessionLabel?: string;
  skillId: string;
  skillTitle: string;
  providerId: ProviderId;
  model: string;
  effort: string;
  prompt: string;
  commandDisplay: string;
  skillRoot: string;
  projectFolder?: string;
  outputDir?: string;
  status: "running" | "completed" | "failed" | "stopped" | "interrupted";
  sessionId: string;
  createdAt: number;
  updatedAt: number;
  finishedAt: number | null;
  resultText: string;
  error: string;
  usage: Record<string, number> | null;
  logs: JobLogEntry[];
}

export interface BatchSessionDraft {
  uid: string;
  sessionLabel: string;
  skillId: string;
  providerId: ProviderId;
  model: string;
  effort: string;
  brief: string;
  topic: string;
  format: string;
  character: string;
  stickMode: string;
  renderStage: "draft" | "final";
  projectFolder: string;
}

export interface TunnelLogLine {
  at: number;
  text: string;
}

export interface ManagedTunnelItem {
  id: string;
  name: string;
  mode: "quick" | "token" | "named";
  targetUrl: string;
  hasToken: boolean;
  tokenPreview: string;
  tunnelNameOrId: string;
  publicHostname: string;
  protocol: "auto" | "http2" | "quic";
  autoStart: boolean;
  createdAt: number;
  updatedAt: number;
  status: "stopped" | "starting" | "running" | "error";
  pid: number | null;
  publicUrl: string;
  startedAt: number | null;
  lastError: string;
  logs: TunnelLogLine[];
}

export interface ExternalTunnelItem {
  pid: number;
  commandLine: string;
  targetHint: string;
}

export interface TunnelsOverview {
  installed: boolean;
  binaryPath: string;
  version: string;
  defaultWebUrl: string;
  managed: ManagedTunnelItem[];
  external: ExternalTunnelItem[];
}

export interface MemoryDocMeta {
  name: string;
  roleVi: string;
  roleEn: string;
  agents: ProviderId[];
  path: string;
  exists: boolean;
  size: number;
  mtimeMs: number;
  preview: string;
}

export interface MemoryHandoffStatus {
  workspaceDir: string;
  exists: boolean;
  files: MemoryDocMeta[];
}

export interface AppSettings {
  skillRoot: string;
  customSkillSaveDir: string;
  extraSkillRoots: string[];
  videoOutputDir: string;
  mediaUploadDir: string;
  theme: ThemeMode;
  language: LangCode;
  autoInjectSharedMemory: boolean;
  autoLogHandoff: boolean;
  syncSkillLinks: boolean;
  selectedSkill: string;
  selectedProvider: ProviderId;
  providerPrefs: {
    claude: { model: string; effort: string; permissionMode: string };
    codex: { model: string; effort: string; sandbox: string };
    antigravity: { model: string; effort: string; permissionMode: string; lsAddress: string; csrfToken: string };
  };
}

export interface BootstrapPayload {
  settings: AppSettings;
  appRoot: string;
  lanUrls: string[];
  platform: string;
  arch: string;
  effortsMeta: Record<string, EffortMeta>;
  providers: ProviderInfo[];
  skillsInfo: {
    skillRoot: string;
    extraRoots?: string[];
    rootExists: boolean;
    hasShared: boolean;
    hasVieNeu: boolean;
    topics: TopicProfile[];
    skills: SkillItem[];
  };
  videos: VideoProjectItem[];
  uploadedMedia: UploadedMediaItem[];
  tunnels: TunnelsOverview;
  memoryStatus: MemoryHandoffStatus;
  jobs: JobItem[];
}

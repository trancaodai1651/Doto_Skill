// Hợp đồng API giữa crates/server (Rust, serde camelCase) và apps/web.
// Mọi thay đổi ở đây PHẢI đồng bộ với crates/server và docs/API.md.

export type Role = 'owner' | 'viewer';

export interface User {
  id: string;
  username: string;
  role: Role;
  createdAt: string;
}

export interface Me {
  user: User | null;
  csrfToken: string | null;
  /** true khi đang chạy trực tiếp trong cửa sổ ứng dụng desktop Tauri (listener Control) */
  hostControl: boolean;
  setupRequired: boolean;
  onboardingCompleted: boolean;
  version: string;
  hostname?: string | null;
  /** Chỉ trả về khi bản Web kết nối chéo miền (authMode = "bearer") */
  sessionToken?: string | null;
}

export type ProfileId = 'eco' | 'balanced' | 'max';

export interface ResourceOverrides {
  maxJobs?: number | null;
  maxCpuThreads?: number | null;
  maxRamMb?: number | null;
  maxGpuJobs?: number | null;
}

export interface ResourceLimits {
  maxJobs: number;
  maxCpuThreads: number;
  maxRamMb: number;
  maxGpuJobs: number;
  ffmpegThreads: number;
  metricsIntervalMs: number;
  /** Balanced: không nhận job mới khi CPU vượt ngưỡng này (%) */
  cpuAdmitThreshold: number;
  /** Không nhận job mới khi RAM khả dụng thấp hơn ngưỡng này (MB) */
  minFreeRamMb: number;
}

export interface ResourceSettings {
  profile: ProfileId;
  overrides: ResourceOverrides;
  effective: ResourceLimits;
}

export interface GpuInfo {
  name: string;
  vendor: string; // nvidia | amd | intel | apple | unknown
  vramMb: number | null;
  driverVersion: string | null;
}

export interface DiskInfo {
  name: string;
  mountPoint: string;
  totalBytes: number;
  availableBytes: number;
  kind: string; // ssd | hdd | unknown
  fileSystem: string;
}

export interface HardwareInfo {
  osName: string;
  osVersion: string;
  arch: string;
  hostname: string;
  cpuBrand: string;
  logicalCores: number;
  physicalCores: number | null;
  totalMemoryMb: number;
  gpus: GpuInfo[];
  disks: DiskInfo[];
  battery: { present: boolean; percent: number | null; charging: boolean | null } | null;
  probedAt: string;
  notes: string[];
}

export interface Metrics {
  ts: string;
  cpuPercent: number;
  memUsedMb: number;
  memTotalMb: number;
  memAvailableMb: number;
  runningJobs: number;
  queuedJobs: number;
  limits: ResourceLimits;
  profile: ProfileId;
  /** Lý do scheduler đang tạm hoãn nhận job (nếu có) */
  throttleReason: string | null;
}

export interface EncoderInfo {
  name: string; // libx264 | h264_nvenc | h264_qsv | h264_amf | h264_videotoolbox | h264_mf
  kind: 'software' | 'hardware';
  vendor: string;
  available: boolean; // có trong `ffmpeg -encoders`
  working: boolean; // đã encode thử thành công
  error: string | null;
}

export interface EncoderBenchmark {
  name: string;
  kind: 'software' | 'hardware' | string;
  vendor: string;
  frames: number;
  elapsedMs: number;
  fps: number;
}

export interface BackupEntry {
  file: string;
  sizeBytes: number;
  createdAt: string;
}

export interface FfmpegStatus {
  found: boolean;
  ffmpegPath: string | null;
  ffprobePath: string | null;
  version: string | null;
  licenseFlags: string[]; // vd ["gpl","nonfree"] — để cảnh báo giấy phép
  encoders: EncoderInfo[];
  encodersProbedAt: string | null;
}

export interface AppSettings {
  language: 'vi' | 'en';
  theme: 'system' | 'light' | 'dark';
  videoSourceDirs: string[];
  outputDir: string | null;
  mediaUploadDir?: string | null;
  customSkillDir?: string | null;
  workspaceMemoryDir?: string | null;
  ffmpegPath: string | null;
  ffprobePath: string | null;
  dataDir: string; // chỉ đọc
  defaultUploadsDir?: string;
  defaultCustomSkillsDir?: string;
  launchAtLogin: boolean;
  runInBackground: boolean;
}

// ---------- Host Directory Browser & Mobile Uploads (P7) ----------
export interface HostDirsResponse {
  current: string | null;
  parent: string | null;
  roots: { label: string; path: string }[];
  entries: { name: string; path: string }[];
}

export interface UploadedMedia {
  id: string;
  originalName: string;
  safeName: string;
  mimeType: string;
  mediaKind: 'video' | 'image';
  sizeBytes: number;
  videoId: string | null;
  createdAt: string;
}

// ---------- Skills & Archive Import (P8) ----------
export type SkillSourceKind = 'folder' | 'github';

export interface SkillSource {
  id: string;
  kind: SkillSourceKind;
  location: string; // đường dẫn thư mục hoặc URL GitHub chuẩn hoá
  displayName: string;
  gitRef: string | null;
  subPath: string | null;
  currentSha: string | null;
  pinned: boolean;
  enabled: boolean;
  watch: boolean;
  private: boolean;
  skillCount: number;
  lastScanAt: string | null;
  lastError: string | null;
  createdAt: string;
}

export interface Skill {
  id: string;
  sourceId: string;
  sourceName: string;
  canonicalId: string;
  name: string;
  description: string;
  relPath: string;
  revision: string; // sha256 nội dung SKILL.md (rút gọn)
  hasScripts: boolean;
  warnings: string[];
  duplicateOf: string[]; // id các skill trùng canonicalId ở nguồn khác
  installedProviders: ProviderId[];
  updatedAt: string;
}

export interface SkillDetail extends Skill {
  content: string; // nội dung SKILL.md (đã giới hạn kích thước)
  files: { path: string; size: number; executable: boolean }[];
  metadata: Record<string, unknown>;
}

export interface ImportedSkillSummary {
  slug: string;
  name: string;
  files: number;
  hasScripts: boolean;
  generatedSkillMd: boolean;
  replaced: boolean;
}

export interface SkillImportReport {
  imported: ImportedSkillSummary[];
  skipped: string[];
  sourceId: string | null;
  targetDir: string;
}

export interface UpdateCheck {
  currentSha: string | null;
  latestSha: string;
  upToDate: boolean;
  changedFiles: { filename: string; status: string; additions: number; deletions: number }[];
  compareUrl: string | null;
}

export interface GithubStatus {
  connected: boolean;
  login: string | null;
  scopes: string[];
  method: 'device' | 'gh' | null;
  deviceFlowAvailable: boolean; // false nếu chưa cấu hình OAuth client_id
  ghCliAvailable: boolean;
}

export interface DeviceFlowStart {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

// ---------- AI CLI tools ----------
export type ProviderId = 'antigravity' | 'codex' | 'claude' | 'cloudflared';
export type AiProviderId = 'antigravity' | 'claude' | 'codex';

export interface InstallMethod {
  id: string;
  label: string;
  command: string; // xem trước lệnh sẽ chạy
  sourceUrl: string; // tài liệu chính thức
  requiresElevation: boolean;
  available: boolean; // ví dụ cần winget/brew/npm có sẵn
  unavailableReason: string | null;
}

export interface ToolStatus {
  id: ProviderId;
  name: string;
  installed: boolean;
  path: string | null;
  version: string | null;
  platformSupported: boolean;
  authStatus: 'logged_in' | 'logged_out' | 'unknown' | 'not_applicable';
  authDetail: string | null;
  installMethods: InstallMethod[];
  docsUrl: string;
  skillsDir: string | null; // nơi provider đọc skills cấp người dùng
  lastCheckedAt: string | null;
}

// ---------- AI Studio (P9) ----------
export interface StudioModelInfo {
  id: string;
  name: string;
  badge: string | null;
  defaultEffort: string;
  efforts: string[];
  slugPattern: string | null;
  desc: string;
}

export interface StudioProviderCatalog {
  id: AiProviderId;
  name: string;
  vendor: string;
  installed: boolean;
  binaryPath: string | null;
  version: string | null;
  skillPrefix: string;
  models: StudioModelInfo[];
}

export type StudioRunMode = 'single' | 'multi_same_ai' | 'multi_ai' | 'custom_batch';

export interface StudioSessionSpec {
  provider: AiProviderId;
  model: string;
  effort: string;
  label?: string;
}

export interface StudioRunRequest {
  mode: StudioRunMode;
  prompt: string;
  title?: string;
  skillId?: string | null;
  provider?: AiProviderId;
  model?: string;
  effort?: string;
  count?: number;
  sessions?: StudioSessionSpec[];
  uploadIds?: string[];
  includeMemoryHint?: boolean;
  visibility?: 'private' | 'viewers';
  confirm: boolean;
}

export interface StudioSession {
  id: string;
  jobId: string;
  sessionIndex: number;
  label: string;
  provider: AiProviderId;
  model: string;
  effort: string;
  skillId: string | null;
  status: JobStatus;
  commandPreview: string;
  outputSubdir: string;
  resultText: string;
  log: string[];
  error: string | null;
  exitCode: number | null;
  outputVideoIds: string[];
  startedAt: string | null;
  finishedAt: string | null;
}

export interface StudioRunDetail {
  job: Job;
  sessions: StudioSession[];
}

// ---------- Web server / network / Multi-Tunnel (P10) ----------
export type ServerMode = 'off' | 'localhost' | 'lan';

export interface ServerStatus {
  running: boolean;
  mode: ServerMode;
  bindAddress: string | null;
  port: number;
  urls: string[];
  lanAddresses: { interface: string; ip: string; scope: 'private' | 'link_local' | 'cgnat' | 'public' }[];
  activeConnections: number;
  startedAt: string | null;
  pid: number;
  lastError: string | null;
}

export interface ServerConfig {
  mode: ServerMode;
  port: number;
  lanInterfaceIp: string | null; // null = 0.0.0.0
  ipAllowlist: string[]; // CIDR, rỗng = cho phép tất cả
  publicBaseUrl: string | null; // vd https://video.example.com khi dùng tunnel
  maxStreamKbps?: number;
  requireCfAccess?: boolean;
  cfAccessTeamDomain?: string | null;
  cfAccessAud?: string | null;
}

export type TunnelMode = 'quick' | 'named';
export type ManagedTunnelKind = 'quick' | 'token' | 'named';

export interface TunnelStatus {
  cloudflaredFound: boolean;
  cloudflaredPath: string | null;
  running: boolean;
  mode: TunnelMode | null;
  publicUrl: string | null;
  hostname: string | null;
  hasToken: boolean;
  autoStart: boolean;
  startedAt: string | null;
  lastError: string | null;
  logTail: string[];
}

export interface TunnelConfig {
  mode: TunnelMode;
  hostname: string | null; // hostname đã cấu hình trên Cloudflare dashboard (named)
  autoStart: boolean;
  token?: string; // chỉ gửi lên khi đặt/đổi; server lưu vào keychain, không bao giờ trả về
}

export interface ManagedTunnelReq {
  name: string;
  kind: ManagedTunnelKind;
  targetUrl: string;
  hostname?: string | null;
  cfTunnelName?: string | null;
  autoStart?: boolean;
  notes?: string;
  token?: string;
}

export interface ManagedTunnel {
  id: string;
  name: string;
  kind: ManagedTunnelKind;
  targetUrl: string;
  hostname: string | null;
  cfTunnelName: string | null;
  hasToken: boolean;
  autoStart: boolean;
  notes: string;
  running: boolean;
  connected: boolean;
  pid: number | null;
  publicUrl: string | null;
  startedAt: string | null;
  lastError: string | null;
  logTail: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ExternalCloudflaredProc {
  pid: number;
  commandLine: string;
  targetHint: string;
}

export interface TunnelsOverview {
  cloudflaredFound: boolean;
  cloudflaredPath: string | null;
  defaultTargetUrl: string;
  managed: ManagedTunnel[];
  external: ExternalCloudflaredProc[];
}

// ---------- Shared Memory & 3-Agent Handoff Center (P11) ----------
export interface MemoryDocStatus {
  id: string;
  relPath: string;
  group: 'shared' | 'handoff' | 'workspace';
  titleVi: string;
  titleEn: string;
  agents: AiProviderId[];
  exists: boolean;
  sizeBytes: number;
  updatedAt: string | null;
  preview: string;
}

export interface HandoffEntry {
  id: number;
  agent: string;
  repoSha: string | null;
  summary: string;
  createdAt: string;
}

export interface MemoryOverview {
  workspaceDir: string;
  isCustomDir: boolean;
  docs: MemoryDocStatus[];
  recentHandoffs: HandoffEntry[];
}

export interface MemoryDocContent {
  id: string;
  relPath: string;
  exists: boolean;
  content: string;
}

// ---------- Devices / sessions ----------
export interface DeviceSession {
  id: string;
  userId: string;
  username: string;
  role: Role;
  deviceName: string;
  userAgent: string | null;
  ip: string | null;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string;
  current: boolean;
  hostControl: boolean;
}

export interface PairingCode {
  code: string; // 8 ký tự, chỉ hiển thị một lần
  expiresAt: string;
  role: Role;
  url: string | null; // URL đăng nhập kèm mã (nếu server đang chạy)
  qrSvg: string | null;
}

// ---------- Video ----------
export type VideoKind = 'source' | 'export';

export interface VideoAsset {
  id: string;
  kind: VideoKind;
  title: string;
  fileName: string;
  sizeBytes: number;
  durationMs: number | null;
  width: number | null;
  height: number | null;
  videoCodec: string | null;
  audioCodec: string | null;
  hasThumbnail: boolean;
  visibility: 'private' | 'viewers';
  status: 'ready' | 'missing' | 'error';
  createdAt: string;
  jobId: string | null;
}

export type JobStatus = 'queued' | 'running' | 'succeeded' | 'failed' | 'canceled';

export interface RenderSegment {
  videoId: string;
  startMs?: number | null;
  endMs?: number | null;
}

export interface RenderRequest {
  title: string;
  segments: RenderSegment[];
  width?: number | null; // mặc định theo nguồn đầu tiên
  height?: number | null;
  fps?: number | null;
  crf?: number | null; // 18–30, mặc định 23
  preset?: string | null; // veryfast|fast|medium|slow
  audioBitrateKbps?: number | null; // mặc định 160
  encoder?: 'auto' | 'software' | string;
  visibility?: 'private' | 'viewers';
}

export interface Job {
  id: string;
  kind: 'render' | 'thumbnail' | 'install' | 'scan' | 'studio';
  status: JobStatus;
  title: string;
  progress: number; // 0..1
  etaSeconds: number | null;
  speed: string | null;
  encoder: string | null;
  error: string | null;
  attempts: number;
  outputVideoId: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface JobDetail extends Job {
  params: Record<string, unknown>;
  log: string[];
}

export interface AuditEvent {
  id: number;
  ts: string;
  actor: string;
  action: string;
  target: string | null;
  detail: Record<string, unknown> | null;
  ip: string | null;
}

export interface ShareLink {
  id: string;
  videoId: string;
  url: string | null;
  createdAt?: string;
  expiresAt: string;
}

// ---------- SSE (/api/events) ----------
export type ServerEvent =
  | { type: 'metrics'; data: Metrics }
  | { type: 'job'; data: Job }
  | { type: 'video'; data: VideoAsset }
  | { type: 'server'; data: ServerStatus }
  | { type: 'tunnel'; data: TunnelStatus }
  | { type: 'tool'; data: ToolStatus }
  | { type: 'skills'; data: { sourceId: string } }
  | { type: 'studio'; data: StudioSession };

export interface ApiError {
  error: string; // mã lỗi máy đọc, vd "unauthorized", "forbidden", "validation"
  message: string; // thông điệp tiếng Việt cho người dùng
}

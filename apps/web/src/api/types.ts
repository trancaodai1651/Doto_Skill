// Hợp đồng API giữa crates/server (Rust, serde camelCase) và apps/web.
// Mọi thay đổi ở đây PHẢI đồng bộ với crates/server/src/dto.rs và docs/API.md.

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
  /** true khi phiên được tạo từ cửa sổ desktop (listener nội bộ) → có quyền điều khiển máy host */
  hostControl: boolean;
  setupRequired: boolean;
  onboardingCompleted: boolean;
  version: string;
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
  ffmpegPath: string | null;
  ffprobePath: string | null;
  dataDir: string; // chỉ đọc
  launchAtLogin: boolean;
  runInBackground: boolean;
}

// ---------- Skills ----------
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

// ---------- Web server / network ----------
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
}

export type TunnelMode = 'quick' | 'named';

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
  kind: 'render' | 'thumbnail' | 'install' | 'scan';
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
  url: string;
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
  | { type: 'skills'; data: { sourceId: string } };

export interface ApiError {
  error: string; // mã lỗi máy đọc, vd "unauthorized", "forbidden", "validation"
  message: string; // thông điệp tiếng Việt cho người dùng
}

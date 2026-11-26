// Mock data for e2e tests — shapes follow src/api/types.ts exactly.
import type {
  AppSettings,
  GithubStatus,
  HardwareInfo,
  Me,
  Metrics,
  ProfileId,
  ResourceLimits,
  ResourceSettings,
  ServerConfig,
  ServerStatus,
  SkillSource,
  ToolStatus,
  User,
  VideoAsset,
} from '../src/api/types';

const NOW = '2026-11-26T08:00:00Z';

export const ownerUser: User = { id: 'u-owner', username: 'chu', role: 'owner', createdAt: NOW };
export const viewerUser: User = { id: 'u-viewer', username: 'xem', role: 'viewer', createdAt: NOW };

export function me(over: Partial<Me> = {}): Me {
  return {
    user: ownerUser,
    csrfToken: 'csrf-test',
    hostControl: true,
    setupRequired: false,
    onboardingCompleted: true,
    version: '0.1.0',
    ...over,
  };
}

export const anonymous: Me = me({ user: null, csrfToken: null, hostControl: false });

export const settings: AppSettings = {
  language: 'vi',
  theme: 'system',
  videoSourceDirs: [],
  outputDir: null,
  ffmpegPath: null,
  ffprobePath: null,
  dataDir: 'C:\\Users\\test\\AppData\\Roaming\\DotoSkill',
  launchAtLogin: false,
  runInBackground: true,
};

export const hardware: HardwareInfo = {
  osName: 'Windows',
  osVersion: '11',
  arch: 'x86_64',
  hostname: 'MAY-TEST',
  cpuBrand: 'Test CPU 8-Core',
  logicalCores: 16,
  physicalCores: 8,
  totalMemoryMb: 32768,
  gpus: [{ name: 'Test GPU', vendor: 'nvidia', vramMb: 8192, driverVersion: '1.0' }],
  disks: [{ name: 'SSD', mountPoint: 'C:\\', totalBytes: 512e9, availableBytes: 200e9, kind: 'ssd', fileSystem: 'NTFS' }],
  battery: null,
  probedAt: NOW,
  notes: [],
};

const LIMITS: Record<ProfileId, ResourceLimits> = {
  eco: { maxJobs: 1, maxCpuThreads: 4, maxRamMb: 4096, maxGpuJobs: 0, ffmpegThreads: 2, metricsIntervalMs: 5000, cpuAdmitThreshold: 50, minFreeRamMb: 4096 },
  balanced: { maxJobs: 2, maxCpuThreads: 12, maxRamMb: 16384, maxGpuJobs: 1, ffmpegThreads: 0, metricsIntervalMs: 2000, cpuAdmitThreshold: 75, minFreeRamMb: 2048 },
  max: { maxJobs: 4, maxCpuThreads: 16, maxRamMb: 28672, maxGpuJobs: 2, ffmpegThreads: 0, metricsIntervalMs: 1000, cpuAdmitThreshold: 95, minFreeRamMb: 1024 },
};

export function resources(profile: ProfileId = 'balanced'): ResourceSettings {
  return { profile, overrides: {}, effective: LIMITS[profile] };
}

export function metrics(profile: ProfileId = 'balanced'): Metrics {
  return {
    ts: NOW,
    cpuPercent: 12,
    memUsedMb: 8000,
    memTotalMb: 32768,
    memAvailableMb: 24000,
    runningJobs: 0,
    queuedJobs: 0,
    limits: LIMITS[profile],
    profile,
    throttleReason: null,
  };
}

export const tools: ToolStatus[] = [
  {
    id: 'claude',
    name: 'Claude Code',
    installed: false,
    path: null,
    version: null,
    platformSupported: true,
    authStatus: 'unknown',
    authDetail: null,
    installMethods: [
      {
        id: 'npm',
        label: 'npm',
        command: 'npm install -g @anthropic-ai/claude-code',
        sourceUrl: 'https://docs.anthropic.com/claude-code',
        requiresElevation: false,
        available: true,
        unavailableReason: null,
      },
    ],
    docsUrl: 'https://docs.anthropic.com/claude-code',
    skillsDir: 'C:\\Users\\test\\.claude\\skills',
    lastCheckedAt: NOW,
  },
  {
    id: 'cloudflared',
    name: 'cloudflared',
    installed: true,
    path: 'C:\\Program Files\\cloudflared\\cloudflared.exe',
    version: '2026.1.0',
    platformSupported: true,
    authStatus: 'not_applicable',
    authDetail: null,
    installMethods: [],
    docsUrl: 'https://developers.cloudflare.com/cloudflare-one/',
    skillsDir: null,
    lastCheckedAt: NOW,
  },
];

export function serverConfig(mode: ServerConfig['mode'] = 'off'): ServerConfig {
  return { mode, port: 4320, lanInterfaceIp: null, ipAllowlist: [], publicBaseUrl: null };
}

export function serverStatus(mode: ServerConfig['mode'] = 'off'): ServerStatus {
  return {
    running: mode !== 'off',
    mode,
    bindAddress: mode === 'lan' ? '0.0.0.0' : mode === 'localhost' ? '127.0.0.1' : null,
    port: 4320,
    urls: mode === 'lan' ? ['http://192.168.1.10:4320'] : [],
    lanAddresses: [{ interface: 'Wi-Fi', ip: '192.168.1.10', scope: 'private' }],
    activeConnections: 0,
    startedAt: mode === 'off' ? null : NOW,
    pid: 1234,
    lastError: null,
  };
}

export const firewallGuide = { os: 'Windows 11', steps: ['Mở Windows Defender Firewall', 'Cho phép cổng 4320'], commands: [] as string[] };

export function folderSource(path: string): SkillSource {
  return {
    id: 'src-1',
    kind: 'folder',
    location: path,
    displayName: 'My Skills',
    gitRef: null,
    subPath: null,
    currentSha: null,
    pinned: false,
    enabled: true,
    watch: true,
    private: false,
    skillCount: 0,
    lastScanAt: NOW,
    lastError: null,
    createdAt: NOW,
  };
}

export const githubStatus: GithubStatus = {
  connected: false,
  login: null,
  scopes: [],
  method: null,
  deviceFlowAvailable: false,
  ghCliAvailable: false,
};

export function video(id: string, title: string, over: Partial<VideoAsset> = {}): VideoAsset {
  return {
    id,
    kind: 'export',
    title,
    fileName: `${title}.mp4`,
    sizeBytes: 150 * 1024 * 1024,
    durationMs: 125_000,
    width: 1920,
    height: 1080,
    videoCodec: 'h264',
    audioCodec: 'aac',
    hasThumbnail: true,
    visibility: 'viewers',
    status: 'ready',
    createdAt: NOW,
    jobId: null,
    ...over,
  };
}

import type {
  AiProviderId,
  AppSettings,
  AuditEvent,
  BackupEntry,
  DeviceFlowStart,
  DeviceSession,
  EncoderBenchmark,
  FfmpegStatus,
  GithubStatus,
  HardwareInfo,
  HostDirsResponse,
  Job,
  JobDetail,
  ManagedTunnel,
  ManagedTunnelReq,
  Me,
  MemoryDocContent,
  MemoryOverview,
  Metrics,
  PairingCode,
  ProfileId,
  ProviderId,
  RenderRequest,
  ResourceOverrides,
  ResourceSettings,
  Role,
  ServerConfig,
  ServerStatus,
  ShareLink,
  Skill,
  SkillDetail,
  SkillImportReport,
  SkillSource,
  StudioProviderCatalog,
  StudioRunDetail,
  StudioRunRequest,
  ToolStatus,
  TunnelConfig,
  TunnelStatus,
  TunnelsOverview,
  UpdateCheck,
  UploadedMedia,
  User,
  VideoAsset,
  VideoKind,
} from './types';
import { storage } from '../lib/storage';

/** Error thrown for any non-2xx response or network failure. Mirrors the server `ApiError` shape. */
export class ApiRequestError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiRequestError';
    this.status = status;
    this.code = code;
  }
}

const FALLBACK_CODES: Record<number, string> = {
  400: 'bad_request',
  401: 'unauthorized',
  403: 'forbidden',
  404: 'not_found',
  409: 'conflict',
  413: 'payload_too_large',
  416: 'range_not_satisfiable',
  429: 'rate_limited',
};

const FALLBACK_MESSAGES: Record<string, string> = {
  unauthorized: 'Bạn cần đăng nhập lại.',
  forbidden: 'Bạn không có quyền thực hiện thao tác này.',
  not_found: 'Không tìm thấy tài nguyên yêu cầu.',
  rate_limited: 'Bạn thao tác quá nhanh. Vui lòng thử lại sau ít phút.',
  network: 'Không kết nối được tới máy chủ Doto Skill (kiểm tra ứng dụng Desktop đang chạy và không ngủ).',
};

/** Build an ApiRequestError from an HTTP status and a parsed (or unparsed) body. */
export function parseApiError(status: number, body: unknown): ApiRequestError {
  if (body && typeof body === 'object') {
    const b = body as Record<string, unknown>;
    if (typeof b.error === 'string') {
      const message = typeof b.message === 'string' && b.message ? b.message : FALLBACK_MESSAGES[b.error] ?? `Lỗi ${status}`;
      return new ApiRequestError(status, b.error, message);
    }
  }
  const code = FALLBACK_CODES[status] ?? (status >= 500 ? 'server_error' : 'http_error');
  const message = FALLBACK_MESSAGES[code] ?? (status >= 500 ? `Lỗi máy chủ (${status}).` : `Yêu cầu thất bại (${status}).`);
  return new ApiRequestError(status, code, message);
}

export function isApiError(e: unknown): e is ApiRequestError {
  return e instanceof ApiRequestError;
}

export function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}

type Method = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

export interface ClientOptions {
  baseUrl?: string;
  fetch?: typeof fetch;
}

export const SERVER_URL_STORAGE_KEY = 'doto.serverUrl';
export const BEARER_STORAGE_KEY = 'doto.bearerToken';

export function normalizeBaseUrl(raw: string | null | undefined): string {
  const s = (raw ?? '').trim().replace(/\/+$/, '');
  if (!s) return '';
  if (typeof window !== 'undefined') {
    try {
      const u = new URL(s);
      if ((u.protocol === 'http:' || u.protocol === 'https:') && u.origin === window.location.origin) {
        return '';
      }
      if (u.protocol === 'http:' || u.protocol === 'https:') {
        return u.origin;
      }
    } catch {
      /* fall through */
    }
  }
  return s;
}

export function createClient(opts: ClientOptions = {}) {
  let baseUrl = normalizeBaseUrl(opts.baseUrl);
  const doFetch = opts.fetch ?? ((input: RequestInfo | URL, init?: RequestInit) => fetch(input, init));
  let csrfToken: string | null = null;
  let bearerToken: string | null = null;
  let mediaTicket: string | null = null;
  const unauthorizedListeners = new Set<() => void>();

  function isCrossOrigin(): boolean {
    if (!baseUrl || typeof window === 'undefined') return Boolean(baseUrl);
    try {
      return new URL(baseUrl, window.location.origin).origin !== window.location.origin;
    } catch {
      return true;
    }
  }

  function mediaUrl(path: string): string {
    if (!baseUrl && !mediaTicket) return path;
    const sep = path.includes('?') ? '&' : '?';
    const withTicket = mediaTicket ? `${path}${sep}ticket=${encodeURIComponent(mediaTicket)}` : path;
    return `${baseUrl}${withTicket}`;
  }

  async function parseResponse<T>(res: Response): Promise<T> {
    const text = await res.text().catch(() => '');
    let parsed: unknown = undefined;
    if (text) {
      const ct = res.headers.get('content-type') ?? '';
      if (ct.includes('json') || /^[[{]/.test(text.trim())) {
        try {
          parsed = JSON.parse(text);
        } catch {
          parsed = text;
        }
      } else {
        parsed = text;
      }
    }
    if (!res.ok) {
      const err = parseApiError(res.status, parsed);
      if (res.status === 401) unauthorizedListeners.forEach((l) => l());
      throw err;
    }
    return parsed as T;
  }

  async function request<T>(method: Method, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
    const cross = isCrossOrigin();
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    if (bearerToken) {
      headers['Authorization'] = `Bearer ${bearerToken}`;
    } else if (method !== 'GET' && csrfToken) {
      headers['x-csrf-token'] = csrfToken;
    }
    let res: Response;
    try {
      res = await doFetch(baseUrl + path, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        credentials: cross || bearerToken ? 'omit' : 'include',
        signal,
      });
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') throw e;
      throw new ApiRequestError(0, 'network', FALLBACK_MESSAGES.network!);
    }
    return parseResponse<T>(res);
  }

  async function postBinary<T>(path: string, body: BodyInit, contentType = 'application/octet-stream', signal?: AbortSignal): Promise<T> {
    const cross = isCrossOrigin();
    const headers: Record<string, string> = {
      Accept: 'application/json',
      'Content-Type': contentType || 'application/octet-stream',
    };
    if (bearerToken) {
      headers['Authorization'] = `Bearer ${bearerToken}`;
    } else if (csrfToken) {
      headers['x-csrf-token'] = csrfToken;
    }
    let res: Response;
    try {
      res = await doFetch(baseUrl + path, {
        method: 'POST',
        headers,
        body,
        credentials: cross || bearerToken ? 'omit' : 'include',
        signal,
      });
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') throw e;
      throw new ApiRequestError(0, 'network', FALLBACK_MESSAGES.network!);
    }
    return parseResponse<T>(res);
  }

  return {
    request,
    postBinary,
    get: <T>(path: string, signal?: AbortSignal) => request<T>('GET', path, undefined, signal),
    post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
    put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
    patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
    del: <T>(path: string) => request<T>('DELETE', path),
    setBaseUrl(next: string) {
      baseUrl = normalizeBaseUrl(next);
    },
    getBaseUrl: () => baseUrl,
    isCrossOrigin,
    mediaUrl,
    setCsrfToken(token: string | null) {
      csrfToken = token;
    },
    getCsrfToken: () => csrfToken,
    setBearerToken(token: string | null) {
      bearerToken = token;
    },
    getBearerToken: () => bearerToken,
    setMediaTicket(ticket: string | null) {
      mediaTicket = ticket;
    },
    getMediaTicket: () => mediaTicket,
    onUnauthorized(listener: () => void) {
      unauthorizedListeners.add(listener);
      return () => {
        unauthorizedListeners.delete(listener);
      };
    },
  };
}

export type HttpClient = ReturnType<typeof createClient>;

/** Encode a query string, skipping empty values. */
export function qs(params: Record<string, string | number | boolean | null | undefined>): string {
  const sp = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === '') continue;
    sp.set(k, String(v));
  }
  const s = sp.toString();
  return s ? `?${s}` : '';
}

const enc = encodeURIComponent;

export function createApi(http: HttpClient) {
  return {
    http,
    health: () => http.get<{ ok: boolean; version: string; uptimeSeconds: number; hostname?: string }>('/api/health'),
    auth: {
      me: () => http.get<Me>('/api/auth/me'),
      setup: (username: string, password: string, authMode?: 'bearer') =>
        http.post<Me | undefined>('/api/auth/setup', { username, password, ...(authMode ? { authMode } : {}) }),
      desktop: (token: string) => http.post<Me | undefined>('/api/auth/desktop', { token }),
      login: (username: string, password: string, deviceName?: string, authMode?: 'bearer') =>
        http.post<Me>('/api/auth/login', { username, password, deviceName, ...(authMode ? { authMode } : {}) }),
      pair: (code: string, deviceName: string, authMode?: 'bearer') =>
        http.post<Me | undefined>('/api/auth/pair', { code, deviceName, ...(authMode ? { authMode } : {}) }),
      ticket: () => http.post<{ ticket: string; expiresIn: number }>('/api/auth/ticket'),
      logout: () => http.post<void>('/api/auth/logout'),
    },
    users: {
      list: () => http.get<User[]>('/api/users'),
      create: (username: string, password: string, role: Role) => http.post<User>('/api/users', { username, password, role }),
      remove: (id: string) => http.del<void>(`/api/users/${enc(id)}`),
      setPassword: (id: string, password: string) => http.post<void>(`/api/users/${enc(id)}/password`, { password }),
    },
    devices: {
      list: () => http.get<DeviceSession[]>('/api/devices'),
      revoke: (id: string) => http.del<void>(`/api/devices/${enc(id)}`),
      createPairing: (role: Role, ttlMinutes: number) => http.post<PairingCode>('/api/pairing', { role, ttlMinutes }),
    },
    fs: {
      dirs: (path?: string) => http.get<HostDirsResponse>(`/api/fs/dirs${qs({ path })}`),
      mkdir: (parent: string, name: string) => http.post<{ ok: boolean; path: string }>('/api/fs/mkdir', { parent, name, confirm: true }),
    },
    uploads: {
      list: () => http.get<UploadedMedia[]>('/api/uploads'),
      upload: (file: File, title?: string) =>
        http.postBinary<{ upload: UploadedMedia; video: VideoAsset | null }>(
          `/api/videos/upload${qs({ filename: file.name, title })}`,
          file,
          file.type || 'application/octet-stream',
        ),
      previewUrl: (id: string) => http.mediaUrl(`/api/uploads/${enc(id)}/preview`),
      remove: (id: string) => http.del<{ ok: boolean }>(`/api/uploads/${enc(id)}`),
    },
    system: {
      hardware: () => http.get<HardwareInfo>('/api/system/hardware'),
      probeHardware: () => http.post<HardwareInfo | undefined>('/api/system/hardware/probe'),
      metrics: () => http.get<Metrics>('/api/system/metrics'),
      ffmpeg: () => http.get<FfmpegStatus>('/api/system/ffmpeg'),
      probeFfmpeg: () => http.post<FfmpegStatus | undefined>('/api/system/ffmpeg/probe'),
      benchmarkFfmpeg: () => http.post<{ results: EncoderBenchmark[]; ranAt: string }>('/api/system/ffmpeg/benchmark'),
      backups: () => http.get<BackupEntry[]>('/api/system/backups'),
      backup: () => http.post<{ ok?: boolean; file?: string; path?: string } | undefined>('/api/system/backup'),
      restore: (file: string) => http.post<{ ok: boolean; file: string }>('/api/system/restore', { file, confirm: true }),
      audit: (limit = 200) => http.get<AuditEvent[]>(`/api/audit${qs({ limit })}`),
    },
    settings: {
      get: () => http.get<AppSettings>('/api/settings'),
      put: (s: AppSettings) => http.put<AppSettings | undefined>('/api/settings', s),
      resources: () => http.get<ResourceSettings>('/api/settings/resources'),
      putResources: (profile: ProfileId, overrides: ResourceOverrides) =>
        http.put<ResourceSettings>('/api/settings/resources', { profile, overrides }),
    },
    onboarding: {
      get: () => http.get<unknown>('/api/onboarding'),
      complete: () => http.post<unknown>('/api/onboarding/complete'),
    },
    skills: {
      list: (q?: string, sourceId?: string) => http.get<Skill[]>(`/api/skills${qs({ q, sourceId })}`),
      get: (id: string) => http.get<SkillDetail>(`/api/skills/${enc(id)}`),
      install: (id: string, provider: ProviderId, allowScripts: boolean) =>
        http.post<unknown>(`/api/skills/${enc(id)}/install`, { provider, confirm: true, allowScripts }),
      uninstall: (id: string, provider: ProviderId) => http.del<unknown>(`/api/skills/${enc(id)}/install/${enc(provider)}`),
      sources: () => http.get<SkillSource[]>('/api/skill-sources'),
      addFolder: (path: string, watch?: boolean) => http.post<SkillSource>('/api/skill-sources/folder', { path, watch }),
      addGithub: (url: string, ref?: string, isPrivate?: boolean) =>
        http.post<SkillSource>('/api/skill-sources/github', { url, ref: ref || undefined, private: isPrivate }),
      importArchive: (file: File, opts: { overwrite?: boolean; autoMd?: boolean }) =>
        http.postBinary<SkillImportReport>(
          `/api/skill-sources/archive${qs({
            filename: file.name,
            overwrite: opts.overwrite ?? false,
            autoMd: opts.autoMd ?? false,
            confirm: true,
          })}`,
          file,
          file.type || 'application/octet-stream',
        ),
      uploadFolder: (payload: {
        folderName: string;
        files: { path: string; contentBase64: string }[];
        overwrite?: boolean;
        autoMd?: boolean;
      }) =>
        http.post<SkillImportReport>('/api/skill-sources/upload-folder', {
          folderName: payload.folderName,
          files: payload.files,
          overwrite: payload.overwrite ?? false,
          autoMd: payload.autoMd ?? false,
          confirm: true,
        }),
      patchSource: (id: string, patch: { enabled?: boolean; watch?: boolean; pinned?: boolean }) =>
        http.patch<SkillSource | undefined>(`/api/skill-sources/${enc(id)}`, patch),
      rescan: (id: string) => http.post<unknown>(`/api/skill-sources/${enc(id)}/rescan`),
      checkUpdate: (id: string) => http.post<UpdateCheck>(`/api/skill-sources/${enc(id)}/check-update`),
      applyUpdate: (id: string, sha: string) => http.post<unknown>(`/api/skill-sources/${enc(id)}/apply-update`, { sha }),
      removeSource: (id: string) => http.del<void>(`/api/skill-sources/${enc(id)}`),
    },
    github: {
      status: () => http.get<GithubStatus>('/api/github/status'),
      deviceStart: () => http.post<DeviceFlowStart>('/api/github/device/start'),
      devicePoll: () => http.post<unknown>('/api/github/device/poll'),
      useGh: () => http.post<GithubStatus | undefined>('/api/github/use-gh', { confirm: true }),
      logout: () => http.post<void>('/api/github/logout'),
    },
    tools: {
      list: () => http.get<ToolStatus[]>('/api/tools'),
      refresh: (id: ProviderId) => http.post<ToolStatus | undefined>(`/api/tools/${enc(id)}/refresh`),
      install: (id: ProviderId, methodId: string) => http.post<Job>(`/api/tools/${enc(id)}/install`, { methodId, confirm: true }),
      diagnostics: (id: ProviderId) => http.get<{ lines: string[] }>(`/api/tools/${enc(id)}/diagnostics`),
    },
    studio: {
      catalog: () => http.get<{ providers: StudioProviderCatalog[] }>('/api/studio/catalog'),
      runs: () => http.get<StudioRunDetail[]>('/api/studio/runs'),
      run: (id: string) => http.get<StudioRunDetail>(`/api/studio/runs/${enc(id)}`),
      start: (req: StudioRunRequest) => http.post<StudioRunDetail>('/api/studio/runs', req),
      cancelRun: (id: string) => http.post<{ ok: boolean; canceled: number }>(`/api/studio/runs/${enc(id)}/cancel`),
      cancelSession: (id: string) => http.post<{ ok: boolean }>(`/api/studio/sessions/${enc(id)}/cancel`),
    },
    server: {
      status: () => http.get<ServerStatus>('/api/server/status'),
      config: () => http.get<ServerConfig>('/api/server/config'),
      putConfig: (c: ServerConfig) => http.put<ServerConfig | undefined>('/api/server/config', c),
      start: () => http.post<ServerStatus | undefined>('/api/server/start'),
      stop: () => http.post<ServerStatus | undefined>('/api/server/stop'),
      restart: () => http.post<ServerStatus | undefined>('/api/server/restart'),
      qrUrl: (url: string) => http.mediaUrl(`/api/server/qr${qs({ url })}`),
      firewallGuide: () => http.get<{ os: string; steps: string[]; commands: string[] }>('/api/server/firewall-guide'),
    },
    tunnel: {
      status: () => http.get<TunnelStatus>('/api/tunnel/status'),
      putConfig: (c: TunnelConfig) => http.put<TunnelStatus | undefined>('/api/tunnel/config', c),
      start: () => http.post<TunnelStatus | undefined>('/api/tunnel/start'),
      stop: () => http.post<TunnelStatus | undefined>('/api/tunnel/stop'),
      deleteToken: () => http.del<void>('/api/tunnel/token'),
    },
    tunnels: {
      overview: () => http.get<TunnelsOverview>('/api/tunnels'),
      create: (req: ManagedTunnelReq) => http.post<ManagedTunnel>('/api/tunnels', req),
      update: (id: string, req: ManagedTunnelReq) => http.patch<ManagedTunnel>(`/api/tunnels/${enc(id)}`, req),
      remove: (id: string) => http.del<{ ok: boolean }>(`/api/tunnels/${enc(id)}`),
      start: (id: string) => http.post<ManagedTunnel>(`/api/tunnels/${enc(id)}/start`),
      stop: (id: string) => http.post<ManagedTunnel>(`/api/tunnels/${enc(id)}/stop`),
      quickWeb: () => http.post<ManagedTunnel>('/api/tunnels/quick-web', { confirm: true }),
      stopExternal: (pid: number) => http.post<{ ok: boolean; pid: number }>(`/api/tunnels/processes/${pid}/stop`, { confirm: true }),
    },
    memory: {
      overview: () => http.get<MemoryOverview>('/api/memory'),
      readDoc: (id: string) => http.get<MemoryDocContent>(`/api/memory/doc${qs({ id })}`),
      writeDoc: (id: string, content: string) => http.put<{ ok: boolean; id: string }>('/api/memory/doc', { id, content, confirm: true }),
      append: (payload: { id?: string; agent?: string; note: string }) => http.post<{ ok: boolean; id: string }>('/api/memory/append', payload),
      init: (overwrite = false) => http.post<{ ok: boolean; created: string[]; kept: string[] }>('/api/memory/init', { overwrite, confirm: true }),
      recordHandoff: (payload: { agent: AiProviderId; summary: string; nextAction?: string }) =>
        http.post<{ ok: boolean }>('/api/memory/handoff', payload),
    },
    videos: {
      list: (kind?: VideoKind | '', q?: string) => http.get<VideoAsset[]>(`/api/videos${qs({ kind, q })}`),
      get: (id: string) => http.get<VideoAsset>(`/api/videos/${enc(id)}`),
      patch: (id: string, patch: { title?: string; visibility?: 'private' | 'viewers' }) =>
        http.patch<VideoAsset | undefined>(`/api/videos/${enc(id)}`, patch),
      remove: (id: string, deleteFile = false) => http.del<void>(`/api/videos/${enc(id)}${deleteFile ? '?deleteFile=true' : ''}`),
      scan: () => http.post<unknown>('/api/videos/scan'),
      thumbnailUrl: (id: string) => http.mediaUrl(`/api/videos/${enc(id)}/thumbnail`),
      streamUrl: (id: string) => http.mediaUrl(`/api/videos/${enc(id)}/stream`),
      downloadUrl: (id: string) => http.mediaUrl(`/api/videos/${enc(id)}/download`),
      shares: (id: string) => http.get<ShareLink[]>(`/api/videos/${enc(id)}/shares`),
      share: (id: string, ttlMinutes: number) => http.post<ShareLink>(`/api/videos/${enc(id)}/share`, { ttlMinutes }),
      revokeShare: (shareId: string) => http.del<void>(`/api/shares/${enc(shareId)}`),
    },
    jobs: {
      list: () => http.get<Job[]>('/api/jobs'),
      get: (id: string) => http.get<JobDetail>(`/api/jobs/${enc(id)}`),
      render: (r: RenderRequest) => http.post<Job>('/api/jobs/render', r),
      cancel: (id: string) => http.post<unknown>(`/api/jobs/${enc(id)}/cancel`),
      retry: (id: string) => http.post<unknown>(`/api/jobs/${enc(id)}/retry`),
    },
  };
}

export type Api = ReturnType<typeof createApi>;

const initialBaseUrl =
  typeof window !== 'undefined'
    ? normalizeBaseUrl(storage.get(SERVER_URL_STORAGE_KEY) ?? (import.meta.env.VITE_API_BASE_URL as string | undefined))
    : '';
export const http = createClient({ baseUrl: initialBaseUrl });
if (typeof window !== 'undefined' && http.isCrossOrigin()) {
  const savedBearer = storage.get(BEARER_STORAGE_KEY);
  if (savedBearer) http.setBearerToken(savedBearer);
}
export const api = createApi(http);

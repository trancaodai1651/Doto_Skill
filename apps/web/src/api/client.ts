import type {
  AppSettings,
  AuditEvent,
  DeviceFlowStart,
  DeviceSession,
  FfmpegStatus,
  GithubStatus,
  HardwareInfo,
  Job,
  JobDetail,
  Me,
  Metrics,
  PairingCode,
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
  SkillSource,
  ToolStatus,
  TunnelConfig,
  TunnelStatus,
  UpdateCheck,
  User,
  VideoAsset,
  VideoKind,
  ProfileId,
} from './types';

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
  not_found: 'Không tìm thấy tài nguyên (hoặc thao tác chỉ có trên cửa sổ ứng dụng desktop).',
  rate_limited: 'Bạn thao tác quá nhanh. Vui lòng thử lại sau ít phút.',
  network: 'Không kết nối được tới máy chủ Doto Skill.',
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

export function createClient(opts: ClientOptions = {}) {
  const baseUrl = opts.baseUrl ?? '';
  const doFetch = opts.fetch ?? ((input: RequestInfo | URL, init?: RequestInit) => fetch(input, init));
  let csrfToken: string | null = null;
  const unauthorizedListeners = new Set<() => void>();

  async function request<T>(method: Method, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    if (method !== 'GET' && csrfToken) headers['x-csrf-token'] = csrfToken;
    let res: Response;
    try {
      res = await doFetch(baseUrl + path, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        credentials: 'include',
        signal,
      });
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') throw e;
      throw new ApiRequestError(0, 'network', FALLBACK_MESSAGES.network!);
    }
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

  return {
    request,
    get: <T>(path: string, signal?: AbortSignal) => request<T>('GET', path, undefined, signal),
    post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
    put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
    patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
    del: <T>(path: string) => request<T>('DELETE', path),
    setCsrfToken(token: string | null) {
      csrfToken = token;
    },
    getCsrfToken: () => csrfToken,
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
    health: () => http.get<{ ok: boolean; version: string; uptimeSeconds: number }>('/api/health'),
    auth: {
      me: () => http.get<Me>('/api/auth/me'),
      setup: (username: string, password: string) => http.post<Me | undefined>('/api/auth/setup', { username, password }),
      desktop: (token: string) => http.post<Me | undefined>('/api/auth/desktop', { token }),
      login: (username: string, password: string, deviceName?: string) =>
        http.post<Me>('/api/auth/login', { username, password, deviceName }),
      pair: (code: string, deviceName: string) => http.post<Me | undefined>('/api/auth/pair', { code, deviceName }),
      logout: () => http.post<void>('/api/auth/logout'),
    },
    users: {
      list: () => http.get<User[]>('/api/users'),
      // Body shape not specified in docs/API.md — assumed { username, password, role }.
      create: (username: string, password: string, role: Role) => http.post<User>('/api/users', { username, password, role }),
      remove: (id: string) => http.del<void>(`/api/users/${enc(id)}`),
      setPassword: (id: string, password: string) => http.post<void>(`/api/users/${enc(id)}/password`, { password }),
    },
    devices: {
      list: () => http.get<DeviceSession[]>('/api/devices'),
      revoke: (id: string) => http.del<void>(`/api/devices/${enc(id)}`),
      createPairing: (role: Role, ttlMinutes: number) => http.post<PairingCode>('/api/pairing', { role, ttlMinutes }),
    },
    system: {
      hardware: () => http.get<HardwareInfo>('/api/system/hardware'),
      probeHardware: () => http.post<HardwareInfo | undefined>('/api/system/hardware/probe'),
      metrics: () => http.get<Metrics>('/api/system/metrics'),
      ffmpeg: () => http.get<FfmpegStatus>('/api/system/ffmpeg'),
      probeFfmpeg: () => http.post<FfmpegStatus | undefined>('/api/system/ffmpeg/probe'),
      backup: () => http.post<{ path?: string } | undefined>('/api/system/backup'),
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
    server: {
      status: () => http.get<ServerStatus>('/api/server/status'),
      config: () => http.get<ServerConfig>('/api/server/config'),
      putConfig: (c: ServerConfig) => http.put<ServerConfig | undefined>('/api/server/config', c),
      start: () => http.post<ServerStatus | undefined>('/api/server/start'),
      stop: () => http.post<ServerStatus | undefined>('/api/server/stop'),
      restart: () => http.post<ServerStatus | undefined>('/api/server/restart'),
      qrUrl: (url: string) => `/api/server/qr${qs({ url })}`,
      firewallGuide: () => http.get<{ os: string; steps: string[]; commands: string[] }>('/api/server/firewall-guide'),
    },
    tunnel: {
      status: () => http.get<TunnelStatus>('/api/tunnel/status'),
      putConfig: (c: TunnelConfig) => http.put<TunnelStatus | undefined>('/api/tunnel/config', c),
      start: () => http.post<TunnelStatus | undefined>('/api/tunnel/start'),
      stop: () => http.post<TunnelStatus | undefined>('/api/tunnel/stop'),
      deleteToken: () => http.del<void>('/api/tunnel/token'),
    },
    videos: {
      list: (kind?: VideoKind | '', q?: string) => http.get<VideoAsset[]>(`/api/videos${qs({ kind, q })}`),
      get: (id: string) => http.get<VideoAsset>(`/api/videos/${enc(id)}`),
      patch: (id: string, patch: { title?: string; visibility?: 'private' | 'viewers' }) =>
        http.patch<VideoAsset | undefined>(`/api/videos/${enc(id)}`, patch),
      remove: (id: string, deleteFile = false) => http.del<void>(`/api/videos/${enc(id)}${deleteFile ? '?deleteFile=true' : ''}`),
      scan: () => http.post<unknown>('/api/videos/scan'),
      thumbnailUrl: (id: string) => `/api/videos/${enc(id)}/thumbnail`,
      streamUrl: (id: string) => `/api/videos/${enc(id)}/stream`,
      downloadUrl: (id: string) => `/api/videos/${enc(id)}/download`,
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

export const http = createClient();
export const api = createApi(http);

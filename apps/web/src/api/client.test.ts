import { describe, expect, it, vi } from 'vitest';
import { ApiRequestError, createApi, createClient, parseApiError, qs } from './client';
import { parseServerEvent } from './events';

function jsonResponse(status: number, body: unknown) {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function mockFetch(res: () => Response) {
  return vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => res());
}

function headersOf(call: unknown[]): Record<string, string> {
  return ((call[1] as RequestInit).headers ?? {}) as Record<string, string>;
}

describe('http client', () => {
  it('sends x-csrf-token on non-GET requests only, with credentials included', async () => {
    const f = mockFetch(() => jsonResponse(200, { ok: true }));
    const http = createClient({ fetch: f });
    http.setCsrfToken('tok-123');

    await http.get('/api/videos');
    await http.post('/api/jobs/render', { title: 'x' });
    await http.put('/api/settings', {});
    await http.del('/api/videos/1');

    expect(headersOf(f.mock.calls[0]!)['x-csrf-token']).toBeUndefined();
    expect(headersOf(f.mock.calls[1]!)['x-csrf-token']).toBe('tok-123');
    expect(headersOf(f.mock.calls[2]!)['x-csrf-token']).toBe('tok-123');
    expect(headersOf(f.mock.calls[3]!)['x-csrf-token']).toBe('tok-123');
    for (const c of f.mock.calls) expect((c[1] as RequestInit).credentials).toBe('include');
    expect((f.mock.calls[1]![1] as RequestInit).body).toBe(JSON.stringify({ title: 'x' }));
  });

  it('omits the csrf header when no token is known', async () => {
    const f = mockFetch(() => jsonResponse(200, {}));
    const http = createClient({ fetch: f });
    await http.post('/api/auth/login', { username: 'a' });
    expect(headersOf(f.mock.calls[0]!)['x-csrf-token']).toBeUndefined();
  });

  it('parses the server ApiError shape into ApiRequestError', async () => {
    const f = mockFetch(() => jsonResponse(403, { error: 'forbidden', message: 'Không có quyền' }));
    const http = createClient({ fetch: f });
    const err = await http.get('/api/settings').catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiRequestError);
    expect(err).toMatchObject({ status: 403, code: 'forbidden', message: 'Không có quyền' });
  });

  it('falls back to status-based codes for non-JSON errors', async () => {
    const f = mockFetch(() => new Response('Bad gateway', { status: 502, headers: { 'content-type': 'text/plain' } }));
    const http = createClient({ fetch: f });
    const err = (await http.get('/api/health').catch((e: unknown) => e)) as ApiRequestError;
    expect(err.code).toBe('server_error');
    expect(err.status).toBe(502);
  });

  it('maps network failures to a "network" ApiRequestError', async () => {
    const f = vi.fn(async () => {
      throw new TypeError('Failed to fetch');
    });
    const http = createClient({ fetch: f });
    const err = (await http.get('/api/health').catch((e: unknown) => e)) as ApiRequestError;
    expect(err.code).toBe('network');
    expect(err.status).toBe(0);
  });

  it('notifies unauthorized listeners on 401', async () => {
    const f = mockFetch(() => jsonResponse(401, { error: 'unauthorized', message: 'x' }));
    const http = createClient({ fetch: f });
    const spy = vi.fn();
    const off = http.onUnauthorized(spy);
    await http.get('/api/jobs').catch(() => undefined);
    expect(spy).toHaveBeenCalledTimes(1);
    off();
    await http.get('/api/jobs').catch(() => undefined);
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('returns undefined for 204 / empty bodies', async () => {
    const f = mockFetch(() => new Response(null, { status: 204 }));
    const http = createClient({ fetch: f });
    await expect(http.post('/api/auth/logout')).resolves.toBeUndefined();
  });

  it('builds typed API URLs with encoded params and query strings', async () => {
    const f = mockFetch(() => jsonResponse(200, []));
    const a = createApi(createClient({ fetch: f }));
    await a.videos.list('export', 'a b');
    await a.skills.list(undefined, 'src/1');
    await a.videos.remove('id/1', true);
    expect(f.mock.calls.map((c) => String(c[0]))).toEqual(['/api/videos?kind=export&q=a+b', '/api/skills?sourceId=src%2F1', '/api/videos/id%2F1?deleteFile=true']);
    expect(a.server.qrUrl('http://192.168.1.2:4320/?pair=AB')).toBe('/api/server/qr?url=http%3A%2F%2F192.168.1.2%3A4320%2F%3Fpair%3DAB');
  });

  it('uses Bearer header, omits credentials, and builds ticketed media URLs when connected to a remote Desktop Server', async () => {
    const f = mockFetch(() => jsonResponse(200, { ok: true }));
    const http = createClient({ baseUrl: 'http://192.168.1.50:4380/', fetch: f });
    http.setBearerToken('sess-xyz');
    http.setMediaTicket('tkt-999');
    const a = createApi(http);

    await a.system.backup();
    expect(String(f.mock.calls[0]![0])).toBe('http://192.168.1.50:4380/api/system/backup');
    expect(headersOf(f.mock.calls[0]!)['Authorization']).toBe('Bearer sess-xyz');
    expect((f.mock.calls[0]![1] as RequestInit).credentials).toBe('omit');
    expect(a.videos.streamUrl('v1')).toBe('http://192.168.1.50:4380/api/videos/v1/stream?ticket=tkt-999');
    expect(a.server.qrUrl('http://192.168.1.50:4380')).toContain('ticket=tkt-999');
  });
});

describe('parseApiError', () => {
  it('uses fallback message when server message is empty', () => {
    const e = parseApiError(429, { error: 'rate_limited', message: '' });
    expect(e.code).toBe('rate_limited');
    expect(e.message.length).toBeGreaterThan(0);
  });
  it('maps 404 without body', () => {
    expect(parseApiError(404, undefined).code).toBe('not_found');
  });
});

describe('qs', () => {
  it('skips empty values', () => {
    expect(qs({ a: '', b: null, c: undefined, d: 0, e: 'x' })).toBe('?d=0&e=x');
    expect(qs({})).toBe('');
  });
});

describe('parseServerEvent', () => {
  it('accepts the {type,data} envelope on the default channel', () => {
    const ev = parseServerEvent('message', JSON.stringify({ type: 'skills', data: { sourceId: 's1' } }));
    expect(ev).toEqual({ type: 'skills', data: { sourceId: 's1' } });
  });
  it('wraps bare payloads from named events', () => {
    const ev = parseServerEvent('video', JSON.stringify({ id: 'v1' }));
    expect(ev).toEqual({ type: 'video', data: { id: 'v1' } });
  });
  it('rejects invalid JSON and unknown types', () => {
    expect(parseServerEvent('message', 'not json')).toBeNull();
    expect(parseServerEvent('message', JSON.stringify({ type: 'nope', data: 1 }))).toBeNull();
  });
});

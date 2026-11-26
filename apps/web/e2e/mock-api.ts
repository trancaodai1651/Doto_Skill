import type { Page, Route } from '@playwright/test';

export interface MockReq {
  method: string;
  path: string;
  query: URLSearchParams;
  params: Record<string, string>;
  body: unknown;
  headers: Record<string, string>;
}

export interface MockRes {
  status?: number;
  json?: unknown;
  body?: string;
  contentType?: string;
}

export type Handler = (req: MockReq) => MockRes;
export type Routes = Record<string, Handler>;

export const ok = (json: unknown): MockRes => ({ status: 200, json });
export const noContent = (): MockRes => ({ status: 204 });

function match(pattern: string, path: string): Record<string, string> | null {
  const a = pattern.split('/').filter(Boolean);
  const b = path.split('/').filter(Boolean);
  if (a.length !== b.length) return null;
  const params: Record<string, string> = {};
  for (let i = 0; i < a.length; i++) {
    const seg = a[i]!;
    if (seg.startsWith(':')) params[seg.slice(1)] = decodeURIComponent(b[i]!);
    else if (seg !== b[i]) return null;
  }
  return params;
}

const THUMB_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><rect width="320" height="180" fill="#334"/></svg>';

/**
 * Intercept every /api/* request with an in-memory handler table, e.g.
 * `{ 'GET /api/auth/me': () => ok(me) }`. Unknown routes return a 404 ApiError.
 * Returns the list of requests seen so tests can assert on calls and bodies.
 */
export async function mockApi(page: Page, routes: Routes) {
  const calls: MockReq[] = [];
  const table = Object.entries(routes).map(([k, h]) => {
    const [method, pattern] = k.split(' ') as [string, string];
    return { method, pattern, h };
  });

  await page.route(
    (url) => url.pathname.startsWith('/api/'),
    async (route: Route) => {
      const req = route.request();
      const url = new URL(req.url());
      let body: unknown = undefined;
      const raw = req.postData();
      if (raw) {
        try {
          body = JSON.parse(raw);
        } catch {
          body = raw;
        }
      }
      const base = { method: req.method(), path: url.pathname, query: url.searchParams, body, headers: req.headers() };

      // Built-ins: SSE is refused (EventSource falls back to polling); media endpoints get tiny stubs.
      if (url.pathname === '/api/events') {
        calls.push({ ...base, params: {} });
        return route.fulfill({ status: 204 });
      }
      if (/^\/api\/videos\/[^/]+\/thumbnail$/.test(url.pathname)) {
        return route.fulfill({ status: 200, contentType: 'image/svg+xml', body: THUMB_SVG });
      }
      if (/^\/api\/videos\/[^/]+\/stream$/.test(url.pathname)) {
        return route.fulfill({ status: 404, contentType: 'application/json', body: '{"error":"not_found","message":"mock"}' });
      }

      for (const r of table) {
        if (r.method !== req.method()) continue;
        const params = match(r.pattern, url.pathname);
        if (!params) continue;
        const full: MockReq = { ...base, params };
        calls.push(full);
        const res = r.h(full);
        if (res.status === 204) return route.fulfill({ status: 204 });
        return route.fulfill({
          status: res.status ?? 200,
          contentType: res.contentType ?? 'application/json',
          body: res.body ?? JSON.stringify(res.json ?? {}),
        });
      }
      calls.push({ ...base, params: {} });
      return route.fulfill({
        status: 404,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'not_found', message: `mock: no route for ${req.method()} ${url.pathname}` }),
      });
    },
  );
  return { calls };
}

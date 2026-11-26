import { createContext, useContext, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { api, http } from './client';
import type { Metrics, ServerEvent } from './types';

export type EventType = ServerEvent['type'];
type EventOf<T extends EventType> = Extract<ServerEvent, { type: T }>;
type Listener = (e: ServerEvent) => void;

export const EVENT_TYPES: EventType[] = ['metrics', 'job', 'video', 'server', 'tunnel', 'tool', 'skills'];

/**
 * Parse one SSE message. Accepts either `data: {"type":"job","data":{…}}` on the
 * default channel, or a named event (`event: job`) whose data is the payload itself
 * or the full envelope. Returns null for anything unrecognised.
 */
export function parseServerEvent(eventName: string, raw: string): ServerEvent | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (parsed && typeof parsed === 'object') {
    const o = parsed as { type?: unknown; data?: unknown };
    if (typeof o.type === 'string' && 'data' in o && (EVENT_TYPES as string[]).includes(o.type)) {
      return o as ServerEvent;
    }
    if ((EVENT_TYPES as string[]).includes(eventName)) {
      return { type: eventName, data: parsed } as ServerEvent;
    }
  }
  return null;
}

export type RealtimeMode = 'connecting' | 'sse' | 'polling' | 'off';

interface EventsValue {
  mode: RealtimeMode;
  latestMetrics: Metrics | null;
  metricsHistory: Metrics[];
  subscribe: (l: Listener) => () => void;
}

const EventsContext = createContext<EventsValue | null>(null);

const MAX_HISTORY = 90;
const POLL_MS = 5000;
const MAX_SSE_ERRORS = 3;

export function EventsProvider({ children, role }: { children: ReactNode; role: 'owner' | 'viewer' | null }) {
  const listeners = useRef(new Set<Listener>());
  const [mode, setMode] = useState<RealtimeMode>(role ? 'connecting' : 'off');
  const [metricsHistory, setHistory] = useState<Metrics[]>([]);

  useEffect(() => {
    if (!role) {
      setMode('off');
      return;
    }
    let disposed = false;
    let es: EventSource | null = null;
    let pollTimer: ReturnType<typeof setInterval> | null = null;
    let errors = 0;

    const dispatch = (ev: ServerEvent) => {
      if (ev.type === 'metrics') setHistory((h) => [...h.slice(-(MAX_HISTORY - 1)), ev.data]);
      listeners.current.forEach((l) => l(ev));
    };

    // ---- polling fallback (e.g. Cloudflare Quick Tunnels do not support SSE) ----
    const seenJobs = new Map<string, string>();
    const seenVideos = new Map<string, string>();
    let videosBaseline = false;
    const poll = async () => {
      const tasks: Promise<void>[] = [];
      if (role === 'owner') {
        tasks.push(api.system.metrics().then((m) => dispatch({ type: 'metrics', data: m })));
        tasks.push(
          api.jobs.list().then((jobs) => {
            for (const j of jobs) {
              const sig = JSON.stringify(j);
              if (seenJobs.get(j.id) !== sig) {
                seenJobs.set(j.id, sig);
                dispatch({ type: 'job', data: j });
              }
            }
          }),
        );
      }
      tasks.push(
        api.videos.list().then((videos) => {
          for (const v of videos) {
            const sig = JSON.stringify(v);
            if (seenVideos.get(v.id) !== sig) {
              seenVideos.set(v.id, sig);
              if (videosBaseline) dispatch({ type: 'video', data: v });
            }
          }
          videosBaseline = true;
        }),
      );
      await Promise.allSettled(tasks);
    };
    const startPolling = () => {
      if (disposed || pollTimer) return;
      es?.close();
      es = null;
      setMode('polling');
      void poll();
      pollTimer = setInterval(() => void poll(), POLL_MS);
    };

    if (typeof EventSource === 'undefined' || (http.isCrossOrigin() && !http.getMediaTicket())) {
      startPolling();
    } else {
      es = new EventSource(http.mediaUrl('/api/events'), { withCredentials: !http.isCrossOrigin() });
      const onMsg = (name: string) => (e: MessageEvent<string>) => {
        const ev = parseServerEvent(name, e.data);
        if (ev) dispatch(ev);
      };
      es.onopen = () => {
        errors = 0;
        setMode('sse');
      };
      es.onmessage = onMsg('message');
      for (const t of EVENT_TYPES) es.addEventListener(t, onMsg(t) as EventListener);
      es.onerror = () => {
        errors++;
        if (!es || es.readyState === EventSource.CLOSED || errors >= MAX_SSE_ERRORS) startPolling();
      };
    }
    return () => {
      disposed = true;
      es?.close();
      if (pollTimer) clearInterval(pollTimer);
    };
  }, [role]);

  const value = useMemo<EventsValue>(
    () => ({
      mode,
      metricsHistory,
      latestMetrics: metricsHistory[metricsHistory.length - 1] ?? null,
      subscribe: (l) => {
        listeners.current.add(l);
        return () => listeners.current.delete(l);
      },
    }),
    [mode, metricsHistory],
  );
  return <EventsContext.Provider value={value}>{children}</EventsContext.Provider>;
}

export function useEvents(): EventsValue {
  const ctx = useContext(EventsContext);
  if (!ctx) throw new Error('useEvents outside EventsProvider');
  return ctx;
}

/** Subscribe to one event type; the handler may change between renders. */
export function useServerEvent<T extends EventType>(type: T, handler: (data: EventOf<T>['data']) => void) {
  const { subscribe } = useEvents();
  const ref = useRef(handler);
  useEffect(() => {
    ref.current = handler;
  });
  useEffect(
    () =>
      subscribe((e) => {
        if (e.type === type) (ref.current as (d: unknown) => void)(e.data);
      }),
    [subscribe, type],
  );
}

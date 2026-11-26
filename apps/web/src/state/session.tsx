import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { BEARER_STORAGE_KEY, SERVER_URL_STORAGE_KEY, api, errorMessage, http, normalizeBaseUrl } from '../api/client';
import type { Me } from '../api/types';
import { readDesktopToken, stripHash } from '../lib/desktop';
import { storage } from '../lib/storage';

interface SessionValue {
  me: Me | null;
  loading: boolean;
  error: string | null;
  /** Error from the desktop token exchange (shown on the login screen). */
  desktopError: string | null;
  /** Địa chỉ máy chủ của bản App đang kết nối ('' = cùng địa chỉ trang web hiện tại). */
  serverUrl: string;
  isCrossOrigin: boolean;
  setServerUrl: (url: string) => Promise<Me | null>;
  refresh: () => Promise<Me | null>;
  setMe: (me: Me) => void;
  logout: () => Promise<void>;
}

const SessionContext = createContext<SessionValue | null>(null);

let bootstrapPromise: Promise<{ desktopError: string | null }> | null = null;

/**
 * One-time desktop bootstrap: if the hash has `desktop-token=…`, exchange it via
 * POST /api/auth/desktop, then strip the hash. Memoised so React StrictMode's
 * double effects never send the one-time token twice.
 */
export function bootstrapDesktop(): Promise<{ desktopError: string | null }> {
  if (!bootstrapPromise) {
    bootstrapPromise = (async () => {
      const token = readDesktopToken(window.location.hash);
      if (!token) return { desktopError: null };
      // Cửa sổ desktop luôn nói chuyện trực tiếp với listener Control cục bộ của chính nó
      http.setBaseUrl('');
      http.setBearerToken(null);
      http.setMediaTicket(null);
      storage.remove(SERVER_URL_STORAGE_KEY);
      storage.remove(BEARER_STORAGE_KEY);
      stripHash();
      try {
        await api.auth.desktop(token);
        return { desktopError: null };
      } catch (e) {
        return { desktopError: errorMessage(e) };
      }
    })();
  }
  return bootstrapPromise;
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [me, setMeState] = useState<Me | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [desktopError, setDesktopError] = useState<string | null>(null);
  const [serverUrl, setServerUrlState] = useState<string>(() => http.getBaseUrl());

  const syncTicketIfNeeded = useCallback(async (m: Me | null) => {
    if (!m?.user || !http.isCrossOrigin()) {
      http.setMediaTicket(null);
      return;
    }
    try {
      const t = await api.auth.ticket();
      http.setMediaTicket(t.ticket);
    } catch {
      http.setMediaTicket(null);
    }
  }, []);

  const setMe = useCallback(
    (m: Me) => {
      if (m.sessionToken) {
        http.setBearerToken(m.sessionToken);
        storage.set(BEARER_STORAGE_KEY, m.sessionToken);
      }
      http.setCsrfToken(m.csrfToken);
      setMeState(m);
      void syncTicketIfNeeded(m);
    },
    [syncTicketIfNeeded],
  );

  const refresh = useCallback(async () => {
    try {
      const m = await api.auth.me();
      if (!m.user && http.getBearerToken()) {
        http.setBearerToken(null);
        http.setMediaTicket(null);
        storage.remove(BEARER_STORAGE_KEY);
      }
      setMe(m);
      setError(null);
      return m;
    } catch (e) {
      setError(errorMessage(e));
      setMeState(null);
      return null;
    }
  }, [setMe]);

  const setServerUrl = useCallback(
    async (raw: string) => {
      const normalized = normalizeBaseUrl(raw);
      http.setBaseUrl(normalized);
      http.setBearerToken(null);
      http.setMediaTicket(null);
      http.setCsrfToken(null);
      storage.remove(BEARER_STORAGE_KEY);
      if (normalized) storage.set(SERVER_URL_STORAGE_KEY, normalized);
      else storage.remove(SERVER_URL_STORAGE_KEY);
      setServerUrlState(normalized);
      setLoading(true);
      try {
        return await refresh();
      } finally {
        setLoading(false);
      }
    },
    [refresh],
  );

  useEffect(() => {
    let alive = true;
    (async () => {
      const { desktopError: de } = await bootstrapDesktop();
      if (!alive) return;
      setServerUrlState(http.getBaseUrl());
      setDesktopError(de);
      await refresh();
      if (alive) setLoading(false);
    })();
    return () => {
      alive = false;
    };
  }, [refresh]);

  // Làm mới media ticket định kỳ khi kết nối chéo miền để stream video / SSE không bị hết hạn
  useEffect(() => {
    if (!me?.user || !http.isCrossOrigin()) return;
    const h = setInterval(() => void syncTicketIfNeeded(me), 8 * 60 * 1000);
    return () => clearInterval(h);
  }, [me, syncTicketIfNeeded]);

  useEffect(() => http.onUnauthorized(() => void refresh()), [refresh]);

  const logout = useCallback(async () => {
    try {
      await api.auth.logout();
    } finally {
      http.setBearerToken(null);
      http.setMediaTicket(null);
      storage.remove(BEARER_STORAGE_KEY);
      await refresh();
    }
  }, [refresh]);

  const isCrossOrigin = http.isCrossOrigin();

  const value = useMemo(
    () => ({ me, loading, error, desktopError, serverUrl, isCrossOrigin, setServerUrl, refresh, setMe, logout }),
    [me, loading, error, desktopError, serverUrl, isCrossOrigin, setServerUrl, refresh, setMe, logout],
  );
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionValue {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error('useSession outside SessionProvider');
  return ctx;
}

/** Capability flags derived from Me — bản Web và bản App có đầy đủ quyền ngang nhau cho Owner. */
export function useCaps() {
  const { me } = useSession();
  const isOwner = me?.user?.role === 'owner';
  const isDesktopWindow = Boolean(me?.hostControl);
  return {
    isOwner,
    isViewer: me?.user?.role === 'viewer',
    /** Đầy đủ chức năng quản trị cho cả cửa sổ Desktop lẫn trình duyệt Web của Owner */
    hostControl: Boolean(isOwner),
    isDesktopWindow,
    remoteOwner: Boolean(isOwner && !isDesktopWindow),
  };
}

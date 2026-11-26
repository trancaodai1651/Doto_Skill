import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { api, errorMessage, http } from '../api/client';
import type { Me } from '../api/types';
import { readDesktopToken, stripHash } from '../lib/desktop';

interface SessionValue {
  me: Me | null;
  loading: boolean;
  error: string | null;
  /** Error from the desktop token exchange (shown on the login screen). */
  desktopError: string | null;
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

  const setMe = useCallback((m: Me) => {
    http.setCsrfToken(m.csrfToken);
    setMeState(m);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const m = await api.auth.me();
      setMe(m);
      setError(null);
      return m;
    } catch (e) {
      setError(errorMessage(e));
      return null;
    }
  }, [setMe]);

  useEffect(() => {
    let alive = true;
    (async () => {
      const { desktopError: de } = await bootstrapDesktop();
      if (!alive) return;
      setDesktopError(de);
      await refresh();
      if (alive) setLoading(false);
    })();
    return () => {
      alive = false;
    };
  }, [refresh]);

  useEffect(() => http.onUnauthorized(() => void refresh()), [refresh]);

  const logout = useCallback(async () => {
    try {
      await api.auth.logout();
    } finally {
      await refresh();
    }
  }, [refresh]);

  const value = useMemo(
    () => ({ me, loading, error, desktopError, refresh, setMe, logout }),
    [me, loading, error, desktopError, refresh, setMe, logout],
  );
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionValue {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error('useSession outside SessionProvider');
  return ctx;
}

/** Capability flags derived from Me. */
export function useCaps() {
  const { me } = useSession();
  const isOwner = me?.user?.role === 'owner';
  return {
    isOwner,
    isViewer: me?.user?.role === 'viewer',
    hostControl: Boolean(isOwner && me?.hostControl),
    remoteOwner: Boolean(isOwner && !me?.hostControl),
  };
}

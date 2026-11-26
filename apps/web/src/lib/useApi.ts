import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { ApiRequestError, errorMessage } from '../api/client';

export interface ApiState<T> {
  data: T | undefined;
  error: string | null;
  errorCode: string | null;
  loading: boolean;
  reload: () => Promise<void>;
  setData: Dispatch<SetStateAction<T | undefined>>;
}

/**
 * Load data with `fn`; re-runs whenever `key` changes. `key = null` skips loading.
 * Stale responses (from an older key) are ignored.
 */
export function useApi<T>(key: string | null, fn: () => Promise<T>): ApiState<T> {
  const [data, setData] = useState<T | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [loading, setLoading] = useState(key !== null);
  const fnRef = useRef(fn);
  useEffect(() => {
    fnRef.current = fn;
  });
  const seq = useRef(0);

  const run = useCallback(async () => {
    const my = ++seq.current;
    setLoading(true);
    try {
      const d = await fnRef.current();
      if (my !== seq.current) return;
      setData(d);
      setError(null);
      setErrorCode(null);
    } catch (e) {
      if (my !== seq.current) return;
      setError(errorMessage(e));
      setErrorCode(e instanceof ApiRequestError ? e.code : null);
    } finally {
      if (my === seq.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (key === null) return;
    void run();
  }, [key, run]);

  return { data, error, errorCode, loading, reload: run, setData };
}

/** Run an async action and track busy/error, returning the result or undefined. */
export function useAction() {
  const [busy, setBusy] = useState<string | null>(null);
  const run = useCallback(async <R,>(id: string, fn: () => Promise<R>): Promise<R> => {
    setBusy(id);
    try {
      return await fn();
    } finally {
      setBusy(null);
    }
  }, []);
  return { busy, run };
}

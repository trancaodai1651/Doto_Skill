import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { storage } from '../lib/storage';

export type ThemePref = 'system' | 'light' | 'dark';

interface ThemeValue {
  theme: ThemePref;
  setTheme: (t: ThemePref) => void;
}

const ThemeContext = createContext<ThemeValue | null>(null);

function isTheme(v: unknown): v is ThemePref {
  return v === 'system' || v === 'light' || v === 'dark';
}

export function applyTheme(t: ThemePref) {
  const root = document.documentElement;
  if (t === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', t);
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemePref>(() => {
    const saved = storage.get('doto.theme');
    return isTheme(saved) ? saved : 'system';
  });
  useEffect(() => applyTheme(theme), [theme]);
  const setTheme = useCallback((t: ThemePref) => {
    storage.set('doto.theme', t);
    setThemeState(t);
  }, []);
  const value = useMemo(() => ({ theme, setTheme }), [theme, setTheme]);
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme outside ThemeProvider');
  return ctx;
}

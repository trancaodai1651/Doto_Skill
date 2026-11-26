import { vi } from './vi';
import { en } from './en';

export type Lang = 'vi' | 'en';
export type TKey = keyof typeof vi;
export type Dict = Partial<Record<string, string>>;
export type Vars = Record<string, string | number>;

export const dictionaries: Record<Lang, Dict> = { vi, en };
export const DEFAULT_LANG: Lang = 'vi';

function interpolate(s: string, vars?: Vars): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

/**
 * Translate `key` in `lang`; falls back to Vietnamese (the source language),
 * then to the key itself so missing strings are visible but never crash.
 */
export function translate(lang: Lang, key: string, vars?: Vars, dicts: Record<Lang, Dict> = dictionaries): string {
  const s = dicts[lang]?.[key] ?? dicts[DEFAULT_LANG]?.[key] ?? key;
  return interpolate(s, vars);
}

export function isLang(v: unknown): v is Lang {
  return v === 'vi' || v === 'en';
}

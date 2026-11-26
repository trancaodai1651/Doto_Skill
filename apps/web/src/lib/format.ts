/** "1:02:03" / "2:05" style duration from milliseconds. Returns "—" for null/invalid. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return '—';
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Binary (1024) byte size: "1.5 GB". */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 ? Math.round(v) : v.toFixed(1).replace(/\.0$/, '')} ${UNITS[i]}`;
}

export function formatMb(mb: number | null | undefined): string {
  if (mb == null) return '—';
  return formatBytes(mb * 1024 * 1024);
}

/** ETA seconds → "1 giờ 2 phút" / "45 giây" (vi) or "1h 2m" (en). */
export function formatEta(seconds: number | null | undefined, lang: 'vi' | 'en' = 'vi'): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return '—';
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (lang === 'en') {
    if (h) return `${h}h ${m}m`;
    if (m) return `${m}m ${sec}s`;
    return `${sec}s`;
  }
  if (h) return `${h} giờ ${m} phút`;
  if (m) return `${m} phút ${sec} giây`;
  return `${sec} giây`;
}

export function formatResolution(w: number | null | undefined, h: number | null | undefined): string {
  if (!w || !h) return '—';
  return `${w}×${h}`;
}

export function formatDate(iso: string | null | undefined, lang: 'vi' | 'en' = 'vi'): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '—';
  return d.toLocaleString(lang === 'vi' ? 'vi-VN' : 'en-US', { dateStyle: 'short', timeStyle: 'short' });
}

export function formatPercent(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '—';
  return `${Math.round(v)}%`;
}

/** Parse "12.5" seconds input to ms, empty → null, invalid → NaN. */
export function secondsToMs(input: string): number | null {
  const t = input.trim();
  if (!t) return null;
  const n = Number(t.replace(',', '.'));
  return Number.isFinite(n) && n >= 0 ? Math.round(n * 1000) : Number.NaN;
}

/** Best-effort friendly device name from the user agent. */
export function guessDeviceName(ua: string = typeof navigator !== 'undefined' ? navigator.userAgent : ''): string {
  const os = /iPhone/.test(ua)
    ? 'iPhone'
    : /iPad/.test(ua)
      ? 'iPad'
      : /Android/.test(ua)
        ? 'Android'
        : /Windows/.test(ua)
          ? 'Windows'
          : /Mac OS X|Macintosh/.test(ua)
            ? 'macOS'
            : /Linux/.test(ua)
              ? 'Linux'
              : 'Thiết bị';
  const browser = /Edg\//.test(ua)
    ? 'Edge'
    : /Firefox\//.test(ua)
      ? 'Firefox'
      : /Chrome\//.test(ua)
        ? 'Chrome'
        : /Safari\//.test(ua)
          ? 'Safari'
          : 'Trình duyệt';
  return `${browser} · ${os}`;
}

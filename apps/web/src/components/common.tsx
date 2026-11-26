import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { Button, EmptyState, Spinner, useToast } from '@doto/ui';
import { errorMessage } from '../api/client';
import { useT } from '../i18n';
import { useCaps } from '../state/session';
import type { ApiState } from '../lib/useApi';

export function PageHeader({ title, description, actions }: { title: ReactNode; description?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-5 flex flex-wrap items-start justify-between gap-3">
      <div className="min-w-0">
        <h1 className="text-xl font-bold md:text-2xl">{title}</h1>
        {description && <p className="mt-1 text-sm text-muted">{description}</p>}
      </div>
      {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
    </header>
  );
}

export function LoadingBlock({ label }: { label?: string }) {
  const t = useT();
  return (
    <div className="flex items-center gap-2 py-8 text-muted" role="status" aria-live="polite">
      <Spinner /> <span>{label ?? t('common.loading')}</span>
    </div>
  );
}

export function ErrorBlock({ message, onRetry }: { message: string; onRetry?: () => void }) {
  const t = useT();
  return (
    <div role="alert" className="rounded-lg border border-danger/40 bg-danger-bg p-4 text-sm">
      <p className="font-semibold text-danger">{t('common.error')}</p>
      <p className="mt-1">{message}</p>
      {onRetry && (
        <Button size="sm" className="mt-3" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      )}
    </div>
  );
}

/** Renders loading / error / empty / content for an ApiState. */
export function AsyncView<T>({
  state,
  isEmpty,
  empty,
  children,
}: {
  state: ApiState<T>;
  isEmpty?: (d: T) => boolean;
  empty?: ReactNode;
  children: (d: T) => ReactNode;
}) {
  if (state.data === undefined) {
    if (state.error) return <ErrorBlock message={state.error} onRetry={() => void state.reload()} />;
    return <LoadingBlock />;
  }
  return (
    <>
      {state.error && <ErrorBlock message={state.error} onRetry={() => void state.reload()} />}
      {isEmpty && isEmpty(state.data) ? empty ?? <EmptyState title="—" /> : children(state.data)}
    </>
  );
}

/** Shows children only on the desktop window (hostControl); otherwise an explanation. */
export function HostOnly({ children, compact }: { children: ReactNode; compact?: boolean }) {
  const { hostControl } = useCaps();
  const t = useT();
  if (hostControl) return <>{children}</>;
  return (
    <span className={compact ? 'text-xs text-muted' : 'inline-block rounded-md bg-surface-2 px-2 py-1 text-xs text-muted'} data-testid="host-only-note">
      🔒 {t('host.only')}
    </span>
  );
}

export function useNotify() {
  const { push } = useToast();
  const t = useT();
  return {
    ok: useCallback((title: string, description?: string) => push({ title, description, tone: 'success' }), [push]),
    info: useCallback((title: string, description?: string) => push({ title, description, tone: 'info' }), [push]),
    fail: useCallback((e: unknown) => push({ title: t('common.error'), description: errorMessage(e), tone: 'danger' }), [push, t]),
  };
}

export function KeyValue({ items }: { items: Array<[ReactNode, ReactNode]> }) {
  return (
    <dl className="grid grid-cols-[minmax(7rem,auto)_1fr] gap-x-4 gap-y-1.5 text-sm">
      {items.map(([k, v], i) => (
        <div key={i} className="contents">
          <dt className="text-muted">{k}</dt>
          <dd className="min-w-0 break-words">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

export function Code({ children }: { children: ReactNode }) {
  return <code className="rounded bg-surface-2 px-1.5 py-0.5 font-mono text-[0.82em] break-all">{children}</code>;
}

export function LogBox({ lines, label }: { lines: string[]; label: string }) {
  const t = useT();
  return (
    <pre
      aria-label={label}
      tabIndex={0}
      className="max-h-80 overflow-auto rounded-md bg-surface-2 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-words"
    >
      {lines.length ? lines.join('\n') : t('common.noLog')}
    </pre>
  );
}

/** Minimal SVG sparkline (values 0..max). */
export function Sparkline({ values, max, label, color = 'var(--doto-primary)' }: { values: number[]; max: number; label: string; color?: string }) {
  const w = 240;
  const h = 56;
  if (values.length < 2) {
    return (
      <svg viewBox={`0 0 ${w} ${h}`} className="h-14 w-full" role="img" aria-label={label}>
        <line x1="0" y1={h - 1} x2={w} y2={h - 1} stroke="var(--doto-line)" />
      </svg>
    );
  }
  const step = w / (values.length - 1);
  const pts = values.map((v, i) => `${(i * step).toFixed(1)},${(h - 2 - (Math.min(v, max) / (max || 1)) * (h - 4)).toFixed(1)}`);
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="h-14 w-full" role="img" aria-label={label} preserveAspectRatio="none">
      <polygon points={`0,${h} ${pts.join(' ')} ${w},${h}`} fill={color} opacity="0.12" />
      <polyline points={pts.join(' ')} fill="none" stroke={color} strokeWidth="2" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

export function QrImage({ src, svg, alt }: { src?: string; svg?: string | null; alt: string }) {
  const url = svg ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` : src;
  if (!url) return null;
  return <img src={url} alt={alt} width={160} height={160} className="rounded-md bg-white p-2" />;
}

export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline underline-offset-2 break-all">
      {children}
    </a>
  );
}

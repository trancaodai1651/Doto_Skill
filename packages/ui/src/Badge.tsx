import type { ReactNode } from 'react';
import { cx } from './cx';

export type Tone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

export function Badge({ tone = 'neutral', children, className }: { tone?: Tone; children: ReactNode; className?: string }) {
  return <span className={cx('ui-badge', tone !== 'neutral' && `ui-badge--${tone}`, className)}>{children}</span>;
}

/** Map common status strings (job/server/tool states) to a tone. */
export function toneForStatus(status: string): Tone {
  switch (status) {
    case 'running':
    case 'succeeded':
    case 'ready':
    case 'logged_in':
    case 'ok':
    case 'installed':
      return 'success';
    case 'queued':
    case 'pending':
    case 'logged_out':
    case 'missing':
      return 'warning';
    case 'failed':
    case 'error':
      return 'danger';
    case 'canceled':
    case 'stopped':
    case 'off':
      return 'neutral';
    default:
      return 'info';
  }
}

export function StatusPill({ status, label, tone }: { status: string; label?: ReactNode; tone?: Tone }) {
  return (
    <span className={cx('ui-badge', `ui-badge--${tone ?? toneForStatus(status)}`)} data-status={status}>
      <span className="ui-badge__dot" aria-hidden="true" />
      {label ?? status}
    </span>
  );
}

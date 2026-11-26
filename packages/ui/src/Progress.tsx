import type { ReactNode } from 'react';
import { cx } from './cx';

export interface ProgressProps {
  /** 0..1, or null for indeterminate */
  value: number | null;
  label?: ReactNode;
  detail?: ReactNode;
  ariaLabel?: string;
  tone?: 'primary' | 'success' | 'danger' | 'warning';
}

export function Progress({ value, label, detail, ariaLabel, tone = 'primary' }: ProgressProps) {
  const pct = value == null ? null : Math.round(Math.min(1, Math.max(0, value)) * 100);
  const color =
    tone === 'success' ? 'var(--doto-success)' : tone === 'danger' ? 'var(--doto-danger)' : tone === 'warning' ? 'var(--doto-warning)' : undefined;
  return (
    <div className="ui-progress">
      <div
        className="ui-progress__track"
        role="progressbar"
        aria-label={ariaLabel ?? (typeof label === 'string' ? label : undefined)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct ?? undefined}
      >
        <div
          className={cx('ui-progress__bar', pct == null && 'ui-progress__bar--indeterminate')}
          style={{ width: pct == null ? undefined : `${pct}%`, background: color }}
        />
      </div>
      {(label || detail || pct != null) && (
        <div className="ui-progress__label">
          <span>{label ?? (pct != null ? `${pct}%` : '')}</span>
          {detail && <span>{detail}</span>}
        </div>
      )}
    </div>
  );
}

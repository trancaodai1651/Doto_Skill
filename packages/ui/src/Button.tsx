import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { cx } from './cx';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: 'sm' | 'md';
  loading?: boolean;
  icon?: ReactNode;
}

export function Spinner({ label }: { label?: string }) {
  return (
    <span className="ui-spinner" role={label ? 'status' : undefined} aria-label={label} aria-hidden={label ? undefined : true} />
  );
}

export function Button({
  variant = 'secondary',
  size = 'md',
  loading = false,
  disabled,
  icon,
  className,
  children,
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cx('ui-btn', `ui-btn--${variant}`, size === 'sm' && 'ui-btn--sm', className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Spinner /> : icon}
      {children}
    </button>
  );
}

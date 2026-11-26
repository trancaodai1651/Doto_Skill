import { useEffect, useId, useRef } from 'react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { cx } from './cx';

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  variant?: 'dialog' | 'drawer';
  wide?: boolean;
  /** role=alertdialog for confirmations */
  alert?: boolean;
  /** Disallow closing via Escape/backdrop (e.g. while busy) */
  dismissable?: boolean;
  closeLabel?: string;
}

/** Focus-trapped modal (dialog or right drawer) rendered in a portal. */
export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  variant = 'dialog',
  wide,
  alert,
  dismissable = true,
  closeLabel = 'Đóng',
}: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const dismissRef = useRef(dismissable);
  dismissRef.current = dismissable;

  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement | null;
    const panel = panelRef.current;
    const focusables = panel ? Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)) : [];
    const preferred = panel?.querySelector<HTMLElement>('[data-autofocus]');
    (preferred ?? focusables[0] ?? panel)?.focus();
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        if (dismissRef.current) {
          e.stopPropagation();
          onCloseRef.current();
        }
        return;
      }
      if (e.key !== 'Tab' || !panelRef.current) return;
      const items = Array.from(panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      );
      if (items.length === 0) {
        e.preventDefault();
        return;
      }
      const first = items[0]!;
      const last = items[items.length - 1]!;
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !panelRef.current.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !panelRef.current.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    }
    document.addEventListener('keydown', onKey, true);
    return () => {
      document.removeEventListener('keydown', onKey, true);
      document.body.style.overflow = prevOverflow;
      previous?.focus?.();
    };
  }, [open]);

  if (!open) return null;
  const isDrawer = variant === 'drawer';
  return createPortal(
    <div
      className={cx('ui-overlay', isDrawer && 'ui-overlay--drawer')}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && dismissable) onClose();
      }}
    >
      <div
        ref={panelRef}
        className={cx(isDrawer ? 'ui-drawer' : 'ui-dialog', wide && !isDrawer && 'ui-dialog--wide')}
        role={alert ? 'alertdialog' : 'dialog'}
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {isDrawer ? (
          <div className="ui-drawer__head">
            <h2 className="ui-dialog__title" id={titleId}>
              {title}
            </h2>
            <button type="button" className="ui-btn ui-btn--ghost ui-btn--sm" onClick={onClose} aria-label={closeLabel}>
              ✕
            </button>
          </div>
        ) : (
          <h2 className="ui-dialog__title" id={titleId}>
            {title}
          </h2>
        )}
        <div className="ui-dialog__body">{children}</div>
        {footer && <div className="ui-dialog__footer">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

export function Drawer(props: Omit<ModalProps, 'variant'>) {
  return <Modal {...props} variant="drawer" />;
}

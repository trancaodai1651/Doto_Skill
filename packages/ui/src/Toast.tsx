import { createContext, useCallback, useContext, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { cx } from './cx';

export type ToastTone = 'info' | 'success' | 'warning' | 'danger';

export interface ToastInput {
  title: ReactNode;
  description?: ReactNode;
  tone?: ToastTone;
  /** ms; 0 = sticky */
  duration?: number;
}

interface ToastItem extends ToastInput {
  id: number;
}

interface ToastApi {
  push: (t: ToastInput) => number;
  dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

export function ToastProvider({ children, closeLabel = 'Đóng thông báo' }: { children: ReactNode; closeLabel?: string }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const seq = useRef(0);

  const dismiss = useCallback((id: number) => {
    setItems((list) => list.filter((t) => t.id !== id));
  }, []);

  const push = useCallback(
    (t: ToastInput) => {
      const id = ++seq.current;
      setItems((list) => [...list.slice(-4), { ...t, id }]);
      const duration = t.duration ?? (t.tone === 'danger' ? 8000 : 5000);
      if (duration > 0) setTimeout(() => dismiss(id), duration);
      return id;
    },
    [dismiss],
  );

  const api = useMemo(() => ({ push, dismiss }), [push, dismiss]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="ui-toasts" aria-live="polite" aria-atomic="false" role="region" aria-label="Thông báo">
        {items.map((t) => (
          <div key={t.id} className={cx('ui-toast', t.tone && t.tone !== 'info' && `ui-toast--${t.tone}`)} role={t.tone === 'danger' ? 'alert' : 'status'}>
            <div>
              <div className="ui-toast__title">{t.title}</div>
              {t.description && <div className="ui-toast__desc">{t.description}</div>}
            </div>
            <button type="button" className="ui-toast__close ui-focusable" aria-label={closeLabel} onClick={() => dismiss(t.id)}>
              ×
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used inside <ToastProvider>');
  return ctx;
}

import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Button } from './Button';
import { Modal } from './Modal';

export interface ConfirmDialogProps {
  open: boolean;
  title: ReactNode;
  children?: ReactNode;
  confirmLabel?: ReactNode;
  cancelLabel?: ReactNode;
  tone?: 'primary' | 'danger';
  /** May return a promise; the dialog shows a spinner until it settles. Throwing keeps the dialog open. */
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
  /** When set, the user must tick this checkbox before confirming (e.g. "Tôi đã xem nội dung"). */
  acknowledge?: ReactNode;
  confirmDisabled?: boolean;
  wide?: boolean;
}

/** Confirmation modal used for every risky action. Focus-trapped, Escape cancels. */
export function ConfirmDialog({
  open,
  title,
  children,
  confirmLabel = 'Xác nhận',
  cancelLabel = 'Huỷ',
  tone = 'primary',
  onConfirm,
  onCancel,
  acknowledge,
  confirmDisabled,
  wide,
}: ConfirmDialogProps) {
  const [busy, setBusy] = useState(false);
  const [acked, setAcked] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setAcked(false);
      setBusy(false);
      setError(null);
    }
  }, [open]);

  async function handleConfirm() {
    setBusy(true);
    setError(null);
    try {
      await onConfirm();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={title}
      alert
      wide={wide}
      dismissable={!busy}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel} disabled={busy} data-autofocus>
            {cancelLabel}
          </Button>
          <Button
            variant={tone === 'danger' ? 'danger' : 'primary'}
            onClick={handleConfirm}
            loading={busy}
            disabled={confirmDisabled || (acknowledge != null && !acked)}
            data-testid="confirm-dialog-confirm"
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
      {acknowledge != null && (
        <label className="ui-check" style={{ marginTop: '1rem' }}>
          <input type="checkbox" checked={acked} onChange={(e) => setAcked(e.target.checked)} />
          <span>{acknowledge}</span>
        </label>
      )}
      {error && (
        <p className="ui-field__error" role="alert" style={{ marginTop: '0.75rem' }}>
          {error}
        </p>
      )}
    </Modal>
  );
}

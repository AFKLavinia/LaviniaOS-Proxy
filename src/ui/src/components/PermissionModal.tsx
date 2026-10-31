import { useEffect } from "react";
import type { Translations } from "../lib/i18n";

interface Props {
  busy: boolean;
  t: Translations;
  onGrant: () => void;
  onDismiss?: () => void;
  onClose?: () => void;
}

export function PermissionModal({ busy, t, onGrant, onDismiss, onClose }: Props) {
  const handleClose = () => {
    if (busy) return;
    if (onClose) onClose();
    else if (onDismiss) onDismiss();
  };

  useEffect(() => {
    const prevActive = document.activeElement as HTMLElement | null;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) {
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      prevActive?.focus?.();
    };
  }, [busy, onClose, onDismiss]);

  return (
    <div className="modal-backdrop" role="presentation" onClick={handleClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="perm-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="perm-title">{t.permTitle}</h2>
        <p>{t.permDesc}</p>
        <p className="modal-note">{t.permNote}</p>
        <div className="modal-actions">
          <button
            type="button"
            className="btn ghost"
            id="perm-later"
            onClick={handleClose}
            disabled={busy}
          >
            {t.permNotNow}
          </button>
          <button
            type="button"
            className="btn primary"
            id="perm-grant"
            onClick={onGrant}
            disabled={busy}
          >
            {busy ? t.permWaiting : t.permGrant}
          </button>
        </div>
      </div>
    </div>
  );
}

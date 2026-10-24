import type { Translations } from "../lib/i18n";

interface Props {
  busy: boolean;
  t: Translations;
  onGrant: () => void;
  onDismiss: () => void;
}

export function PermissionModal({ busy, t, onGrant, onDismiss }: Props) {
  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="perm-title">
        <h2 id="perm-title">{t.permTitle}</h2>
        <p>{t.permDesc}</p>
        <p className="modal-note">{t.permNote}</p>
        <div className="modal-actions">
          <button type="button" className="btn ghost" id="perm-later" onClick={onDismiss} disabled={busy}>
            {t.permNotNow}
          </button>
          <button type="button" className="btn primary" id="perm-grant" onClick={onGrant} disabled={busy}>
            {busy ? t.permWaiting : t.permGrant}
          </button>
        </div>
      </div>
    </div>
  );
}

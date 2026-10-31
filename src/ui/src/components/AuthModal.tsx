import { useEffect, useState } from "react";
import type { Translations } from "../lib/i18n";

interface Props {
  host: string;
  port: string;
  t: Translations;
  onConfirm: (user: string, pass: string) => void;
  onDismiss?: () => void;
  onCancel?: () => void;
}

export function AuthModal({ host, port, t, onConfirm, onDismiss, onCancel }: Props) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  const handleDismiss = () => {
    if (onCancel) onCancel();
    else if (onDismiss) onDismiss();
  };

  useEffect(() => {
    const prevActive = document.activeElement as HTMLElement | null;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleDismiss();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      prevActive?.focus?.();
    };
  }, [onCancel, onDismiss]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (username.trim() && password.length > 0) {
      onConfirm(username.trim(), password);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onClick={handleDismiss}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="auth-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="auth-title">{t.authTitle}</h2>
        <p>
          <code>{host}:{port}</code> {t.authDesc}
        </p>
        <form onSubmit={handleSubmit} className="auth-form">
          <label className="field">
            <span>{t.authUsername}</span>
            <input
              id="auth-username"
              type="text"
              required
              autoFocus
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={t.authUsername}
            />
          </label>
          <label className="field">
            <span>{t.authPassword}</span>
            <input
              id="auth-password"
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
            />
          </label>
          <div className="modal-actions">
            <button type="button" className="btn ghost" onClick={handleDismiss}>
              {t.authCancel}
            </button>
            <button type="submit" className="btn primary">
              {t.authConnect}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

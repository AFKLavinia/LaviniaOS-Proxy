import { useState } from "react";
import type { Translations } from "../lib/i18n";

interface Props {
  host: string;
  port: string;
  t: Translations;
  onConfirm: (user: string, pass: string) => void;
  onDismiss: () => void;
}

export function AuthModal({ host, port, t, onConfirm, onDismiss }: Props) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (username.trim() && password.trim()) {
      onConfirm(username.trim(), password.trim());
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="auth-title">
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
            <button type="button" className="btn ghost" onClick={onDismiss}>
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

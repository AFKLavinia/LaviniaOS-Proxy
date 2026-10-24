import type { Phase } from "../ipc";
import type { Translations } from "../lib/i18n";

interface Props {
  phase: Phase;
  t: Translations;
  onOpenSettings: () => void;
}

export function Header({ phase, t, onOpenSettings }: Props) {
  const on = phase === "running";
  const degraded = phase === "degraded";
  return (
    <header className="header">
      <div className="brand">
        <span className="logo" aria-hidden="true" />
        <div>
          <h1 className="title">{t.appName}</h1>
          <p className="subtitle">{t.appSubtitle}</p>
        </div>
      </div>
      <div className="header-actions">
        <span className={on ? "pill pill-on" : degraded ? "pill pill-warn" : "pill"} id="status-pill">
          <i className="dot" />
          {on ? t.statusActive : degraded ? t.statusDegraded : t.statusIdle}
        </span>
        <button
          type="button"
          className="settings-trigger-btn"
          id="btn-settings"
          onClick={onOpenSettings}
          title={t.settingsTitle}
          aria-label={t.settingsTitle}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>
    </header>
  );
}

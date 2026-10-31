import { useEffect, useState } from "react";
import { ipc } from "../ipc";
import type { Lang, Translations } from "../lib/i18n";

interface Props {
  t: Translations & { autostartLabel?: string };
  lang: Lang;
  theme: string;
  onLangChange: (lang: Lang) => void;
  onThemeChange: (theme: string) => void;
  onClose: () => void;
}

const autostartIpc = ipc as typeof ipc & {
  getAutostart?: () => Promise<boolean>;
  setAutostart?: (enabled: boolean) => Promise<boolean>;
};

export function SettingsModal({ t, lang, theme, onLangChange, onThemeChange, onClose }: Props) {
  const [autostart, setAutostart] = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(false);

  useEffect(() => {
    let active = true;
    if (typeof autostartIpc.getAutostart === "function") {
      autostartIpc
        .getAutostart()
        .then((enabled) => {
          if (active) setAutostart(Boolean(enabled));
        })
        .catch(() => {});
    }
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const prevActive = document.activeElement as HTMLElement | null;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      prevActive?.focus?.();
    };
  }, [onClose]);

  const handleToggleAutostart = async () => {
    const next = !autostart;
    setAutostart(next);
    if (typeof autostartIpc.setAutostart === "function") {
      setAutostartBusy(true);
      try {
        const res = await autostartIpc.setAutostart(next);
        setAutostart(Boolean(res));
      } catch {
        setAutostart(!next);
      } finally {
        setAutostartBusy(false);
      }
    }
  };

  const autostartLabel = t.autostartLabel || (lang === "tr" ? "Başlangıçta Otomatik Başlat" : "Launch on system startup");

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="modal settings-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <div className="settings-header-title">
            <svg
              viewBox="0 0 24 24"
              width="20"
              height="20"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
            <h2 id="settings-title">{t.settingsTitle}</h2>
          </div>
          <button type="button" className="icon-btn-ghost" onClick={onClose} aria-label={t.close}>
            <svg
              viewBox="0 0 24 24"
              width="18"
              height="18"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              aria-hidden="true"
            >
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="settings-body">
          <div className="settings-group">
            <span id="lang-label" className="settings-label">{t.language}</span>
            <div className="segmented-control" role="radiogroup" aria-labelledby="lang-label">
              <button
                type="button"
                role="radio"
                aria-checked={lang === "tr"}
                className={`segment-btn ${lang === "tr" ? "active" : ""}`}
                onClick={() => onLangChange("tr")}
              >
                Türkçe 🇹🇷
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={lang === "en"}
                className={`segment-btn ${lang === "en" ? "active" : ""}`}
                onClick={() => onLangChange("en")}
              >
                English 🇬🇧
              </button>
            </div>
          </div>

          <div className="settings-group">
            <span id="theme-label" className="settings-label">{t.theme}</span>
            <div className="theme-options" role="radiogroup" aria-labelledby="theme-label">
              <button
                type="button"
                role="radio"
                aria-checked={theme === "default"}
                className={`theme-badge ${theme === "default" ? "active" : ""}`}
                onClick={() => onThemeChange("default")}
              >
                <span className="theme-color-preview default" aria-hidden="true" />
                <span>{t.themeDark}</span>
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={theme === "midnight"}
                className={`theme-badge ${theme === "midnight" ? "active" : ""}`}
                onClick={() => onThemeChange("midnight")}
              >
                <span className="theme-color-preview midnight" aria-hidden="true" />
                <span>{t.themeMidnight}</span>
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={theme === "oled"}
                className={`theme-badge ${theme === "oled" ? "active" : ""}`}
                onClick={() => onThemeChange("oled")}
              >
                <span className="theme-color-preview oled" aria-hidden="true" />
                <span>{t.themeOled}</span>
              </button>
            </div>
          </div>

          <div className="settings-group">
            <div className="settings-toggle-row">
              <span id="autostart-label" className="settings-label">{autostartLabel}</span>
              <button
                id="autostart-toggle"
                type="button"
                role="switch"
                aria-checked={autostart}
                aria-labelledby="autostart-label"
                className={`toggle-switch ${autostart ? "active" : ""}`}
                disabled={autostartBusy}
                onClick={handleToggleAutostart}
              >
                <span className="toggle-slider" />
              </button>
            </div>
          </div>
        </div>

        <div className="modal-actions">
          <button type="button" className="btn primary" onClick={onClose}>
            {t.close}
          </button>
        </div>
      </div>
    </div>
  );
}

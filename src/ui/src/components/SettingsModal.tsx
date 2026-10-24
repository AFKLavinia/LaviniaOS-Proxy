import type { Lang, Translations } from "../lib/i18n";

interface Props {
  t: Translations;
  lang: Lang;
  theme: string;
  onLangChange: (lang: Lang) => void;
  onThemeChange: (theme: string) => void;
  onClose: () => void;
}

export function SettingsModal({ t, lang, theme, onLangChange, onThemeChange, onClose }: Props) {
  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div className="modal settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <div className="settings-header-title">
            <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
            <h2 id="settings-title">{t.settingsTitle}</h2>
          </div>
          <button type="button" className="icon-btn-ghost" onClick={onClose} aria-label={t.close}>
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="settings-body">
          <div className="settings-group">
            <label className="settings-label">{t.language}</label>
            <div className="segmented-control">
              <button
                type="button"
                className={`segment-btn ${lang === "tr" ? "active" : ""}`}
                onClick={() => onLangChange("tr")}
              >
                Türkçe 🇹🇷
              </button>
              <button
                type="button"
                className={`segment-btn ${lang === "en" ? "active" : ""}`}
                onClick={() => onLangChange("en")}
              >
                English 🇬🇧
              </button>
            </div>
          </div>

          <div className="settings-group">
            <label className="settings-label">{t.theme}</label>
            <div className="theme-options">
              <button
                type="button"
                className={`theme-badge ${theme === "default" ? "active" : ""}`}
                onClick={() => onThemeChange("default")}
              >
                <span className="theme-color-preview default" />
                <span>{t.themeDark}</span>
              </button>
              <button
                type="button"
                className={`theme-badge ${theme === "midnight" ? "active" : ""}`}
                onClick={() => onThemeChange("midnight")}
              >
                <span className="theme-color-preview midnight" />
                <span>{t.themeMidnight}</span>
              </button>
              <button
                type="button"
                className={`theme-badge ${theme === "oled" ? "active" : ""}`}
                onClick={() => onThemeChange("oled")}
              >
                <span className="theme-color-preview oled" />
                <span>{t.themeOled}</span>
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

import type { Phase } from "../ipc";
import type { Translations } from "../lib/i18n";

interface Props {
  phase: Phase;
  busy: boolean;
  t: Translations;
  onToggle: () => void;
}

export function PowerButton({ phase, busy, t, onToggle }: Props) {
  const on = phase === "running";
  const degraded = phase === "degraded";
  const engaged = on || degraded;
  return (
    <div className="power-wrap">
      <div className={`power-ring ${on ? "active" : degraded ? "warn" : ""}`}>
        <button
          id="tunnel-toggle"
          type="button"
          className={on ? "power on" : degraded ? "power warn" : "power"}
          disabled={busy}
          onClick={onToggle}
          aria-pressed={engaged}
          aria-label={engaged ? t.tapToStop : t.tapToStart}
        >
          <svg viewBox="0 0 24 24" width="44" height="44" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
            <path d="M12 3v9" />
            <path d="M6.3 6.8a8 8 0 1 0 11.4 0" />
          </svg>
        </button>
      </div>
      <span className="power-label">
        {degraded ? t.tapToStopBlocked : on ? t.tapToStop : t.tapToStart}
      </span>
    </div>
  );
}

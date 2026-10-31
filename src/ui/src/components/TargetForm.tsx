import { useState } from "react";
import type { Target } from "../ipc";
import type { Translations } from "../lib/i18n";

interface Props {
  host: string;
  port: string;
  disabled: boolean;
  t: Translations;
  onChange: (next: { host: string; port: string }) => void;
}

export function TargetForm({ host, port, disabled, t, onChange }: Props) {
  const [savedList, setSavedList] = useState<string[]>(() => {
    try {
      const stored = localStorage.getItem("lavinia_saved_endpoints");
      if (!stored) return [];
      const parsed = JSON.parse(stored);
      if (Array.isArray(parsed) && parsed.every((x) => typeof x === "string")) {
        return parsed;
      }
      return [];
    } catch {
      return [];
    }
  });

  const handleSaveCurrent = () => {
    const trimmedH = host.trim();
    const trimmedP = port.trim();
    if (!trimmedH || !trimmedP) return;
    const ep = `${trimmedH}:${trimmedP}`;
    if (!savedList.includes(ep)) {
      const updated = [ep, ...savedList].slice(0, 5);
      setSavedList(updated);
      try {
        localStorage.setItem("lavinia_saved_endpoints", JSON.stringify(updated));
      } catch {
        // ignore storage errors
      }
    }
  };

  const handleSelect = (ep: string) => {
    const idx = ep.lastIndexOf(":");
    if (idx > 0) {
      const h = ep.slice(0, idx);
      const p = ep.slice(idx + 1);
      if (h && p) {
        onChange({ host: h, port: p });
      }
    }
  };

  const handleDelete = (ep: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const updated = savedList.filter((item) => item !== ep);
    setSavedList(updated);
    try {
      localStorage.setItem("lavinia_saved_endpoints", JSON.stringify(updated));
    } catch {
      // ignore storage errors
    }
  };

  return (
    <section className="card" aria-label={t.proxyEndpoint}>
      <div className="card-header-row">
        <h2 className="card-title">{t.proxyEndpoint}</h2>
        {host.trim() && port.trim() && !disabled && (
          <button
            type="button"
            className="card-action-btn"
            onClick={handleSaveCurrent}
            title={t.saveCurrent}
          >
            + {t.saveCurrent}
          </button>
        )}
      </div>

      <div className="fields">
        <label className="field grow">
          <span>{t.ipAddress}</span>
          <input
            id="target-host"
            value={host}
            disabled={disabled}
            inputMode="url"
            spellCheck={false}
            placeholder="192.168.43.1"
            onChange={(e) => onChange({ host: e.target.value, port })}
          />
        </label>
        <label className="field port">
          <span>{t.port}</span>
          <input
            id="target-port"
            value={port}
            disabled={disabled}
            inputMode="numeric"
            spellCheck={false}
            placeholder="1080"
            onChange={(e) => {
              const digits = e.target.value.replace(/\D/g, "");
              if (!digits) {
                onChange({ host, port: "" });
                return;
              }
              const num = Number.parseInt(digits, 10);
              const clamped = num > 65535 ? 65535 : num;
              onChange({ host, port: String(clamped) });
            }}
          />
        </label>
      </div>

      {savedList.length > 0 && !disabled && (
        <div className="quick-endpoints">
          <span className="quick-label">{t.savedEndpoints}:</span>
          <div className="quick-tags">
            {savedList.map((ep) => (
              <button
                type="button"
                key={ep}
                className={`quick-tag ${ep === `${host.trim()}:${port.trim()}` ? "active" : ""}`}
                onClick={() => handleSelect(ep)}
              >
                {ep}
                <button
                  type="button"
                  className="quick-del-btn"
                  title={t.delete}
                  aria-label={`${t.delete} ${ep}`}
                  onClick={(e) => handleDelete(ep, e)}
                >
                  ×
                </button>
              </button>
            ))}
          </div>
        </div>
      )}

      <p className="hint">{t.endpointHint}</p>
    </section>
  );
}

export function toTarget(host: string, port: string): Target {
  const n = Number.parseInt(port.trim(), 10);
  const validPort = Number.isInteger(n) && n >= 1 && n <= 65535 ? n : 0;
  return { host: host.trim(), port: validPort };
}

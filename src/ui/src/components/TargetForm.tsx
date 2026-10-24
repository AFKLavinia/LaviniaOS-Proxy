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
      return stored ? JSON.parse(stored) : [];
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
      localStorage.setItem("lavinia_saved_endpoints", JSON.stringify(updated));
    }
  };

  const handleSelect = (ep: string) => {
    const [h, p] = ep.split(":");
    if (h && p) {
      onChange({ host: h, port: p });
    }
  };

  const handleDelete = (ep: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const updated = savedList.filter((item) => item !== ep);
    setSavedList(updated);
    localStorage.setItem("lavinia_saved_endpoints", JSON.stringify(updated));
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
            inputMode="decimal"
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
            onChange={(e) => onChange({ host, port: e.target.value.replace(/\D/g, "").slice(0, 5) })}
          />
        </label>
      </div>

      {savedList.length > 0 && !disabled && (
        <div className="quick-endpoints">
          <span className="quick-label">{t.savedEndpoints}:</span>
          <div className="quick-tags">
            {savedList.map((ep) => (
              <span
                key={ep}
                className={`quick-tag ${ep === `${host.trim()}:${port.trim()}` ? "active" : ""}`}
                onClick={() => handleSelect(ep)}
              >
                {ep}
                <button
                  type="button"
                  className="quick-del-btn"
                  title={t.delete}
                  onClick={(e) => handleDelete(ep, e)}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        </div>
      )}

      <p className="hint">{t.endpointHint}</p>
    </section>
  );
}

export function toTarget(host: string, port: string): Target {
  return { host: host.trim(), port: Number(port) };
}

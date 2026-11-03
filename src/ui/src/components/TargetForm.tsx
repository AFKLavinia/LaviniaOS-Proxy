import { memo, useState } from "react";
import type { Target } from "../ipc";
import type { Translations } from "../lib/i18n";

interface Props {
  host: string;
  port: string;
  disabled: boolean;
  t: Translations;
  onChange: (next: { host: string; port: string }) => void;
  onCredentialsDetected?: (user: string, pass: string) => void;
}

/**
 * Robust endpoint parser handling URLs, scheme prefixes, Telegram proxy links,
 * credentials extraction and port validation.
 */
export function parseEndpoint(input: string): { host: string; port?: string; user?: string; pass?: string } | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  // Handle Telegram socks proxy links: tg://socks?server=1.2.3.4&port=1080&user=u&pass=p
  if (trimmed.startsWith("tg://") || trimmed.startsWith("tg:socks")) {
    try {
      const url = new URL(trimmed.replace(/^tg:\/\/socks\?/, "https://dummy/?"));
      const server = url.searchParams.get("server");
      const port = url.searchParams.get("port");
      const user = url.searchParams.get("user") ?? undefined;
      const pass = url.searchParams.get("pass") ?? undefined;
      if (server) {
        return { host: server, port: port ?? "1080", user, pass };
      }
    } catch {
      // Fallback to regex if URL parser fails
    }
  }

  // Handle scheme prefixes (socks5://, http://, etc.)
  let clean = trimmed.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "");

  // Extract credentials if present: user:pass@host:port
  let user: string | undefined;
  let pass: string | undefined;
  const atIdx = clean.indexOf("@");
  if (atIdx > 0) {
    const creds = clean.slice(0, atIdx);
    clean = clean.slice(atIdx + 1);
    const colonIdx = creds.indexOf(":");
    if (colonIdx >= 0) {
      user = creds.slice(0, colonIdx);
      pass = creds.slice(colonIdx + 1);
    } else {
      user = creds;
    }
  }

  // Strip trailing path/slash
  clean = clean.split("/")[0].split("?")[0].split("#")[0];

  // IPv6 bracketed format: [2001:db8::1]:1080
  const ipv6Match = clean.match(/^\[([0-9a-fA-F:]+)\](?::([0-9]+))?$/);
  if (ipv6Match) {
    return {
      host: ipv6Match[1],
      port: ipv6Match[2],
      user,
      pass,
    };
  }

  // Host:port format
  const lastColon = clean.lastIndexOf(":");
  if (lastColon > 0) {
    const h = clean.slice(0, lastColon);
    const p = clean.slice(lastColon + 1);
    if (/^\d+$/.test(p)) {
      return { host: h, port: p, user, pass };
    }
  }

  return { host: clean, user, pass };
}

export const TargetForm = memo(function TargetForm({
  host,
  port,
  disabled,
  t,
  onChange,
  onCredentialsDetected,
}: Props) {
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

  const handleHostPaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    const text = e.clipboardData.getData("text");
    const parsed = parseEndpoint(text);
    if (parsed) {
      e.preventDefault();
      const nextPort = parsed.port ? String(Math.min(65535, Math.max(1, parseInt(parsed.port, 10)))) : port;
      onChange({ host: parsed.host, port: nextPort });
      if (parsed.user && parsed.pass && onCredentialsDetected) {
        onCredentialsDetected(parsed.user, parsed.pass);
      }
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
            onPaste={handleHostPaste}
            onChange={(e) => {
              const val = e.target.value;
              // If user pastes/types a full URL or host:port into the host field
              if (val.includes("://") || val.includes("tg:")) {
                const parsed = parseEndpoint(val);
                if (parsed) {
                  const nextPort = parsed.port ? String(Math.min(65535, Math.max(1, parseInt(parsed.port, 10)))) : port;
                  onChange({ host: parsed.host, port: nextPort });
                  if (parsed.user && parsed.pass && onCredentialsDetected) {
                    onCredentialsDetected(parsed.user, parsed.pass);
                  }
                  return;
                }
              }
              onChange({ host: val, port });
            }}
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
              <div
                key={ep}
                className={`quick-tag-wrap ${ep === `${host.trim()}:${port.trim()}` ? "active" : ""}`}
                style={{ display: "inline-flex", alignItems: "center" }}
              >
                <button
                  type="button"
                  className="quick-tag"
                  onClick={() => handleSelect(ep)}
                >
                  {ep}
                </button>
                <button
                  type="button"
                  className="quick-del-btn"
                  title={t.delete}
                  aria-label={`${t.delete} ${ep}`}
                  onClick={(e) => handleDelete(ep, e)}
                  style={{ marginLeft: "-6px", zIndex: 2 }}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      <p className="hint">{t.endpointHint}</p>
    </section>
  );
});

export function toTarget(host: string, port: string): Target {
  const n = Number.parseInt(port.trim(), 10);
  const validPort = Number.isInteger(n) && n >= 1 && n <= 65535 ? n : 0;
  return { host: host.trim(), port: validPort };
}

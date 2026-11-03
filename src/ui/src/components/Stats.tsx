import { formatCount, formatRate } from "../lib/format";
import type { Telemetry } from "../ipc";
import type { Translations } from "../lib/i18n";

export const HISTORY = 48;

function Sparkline({ values, tone }: { values: number[]; tone: "rx" | "tx" }) {
  const w = 200;
  const h = 44;
  if (values.length === 0) {
    return (
      <svg className={`spark ${tone}`} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden="true" />
    );
  }
  const max = Math.max(1, ...values);
  const step = w / (HISTORY - 1);
  const pts = values.map((v, i) => `${(i + HISTORY - values.length) * step},${h - (v / max) * (h - 4) - 2}`);
  const line = pts.join(" ");
  const area = `${(HISTORY - values.length) * step},${h} ${line} ${w},${h}`;
  return (
    <svg className={`spark ${tone}`} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden="true">
      <polygon points={area} className="spark-fill" />
      <polyline points={line} className="spark-line" />
    </svg>
  );
}

interface Props {
  latest: Telemetry;
  rxHistory: number[];
  txHistory: number[];
  t: Translations & { statsTelemetry?: string };
}

export function Stats({ latest, rxHistory, txHistory, t }: Props) {
  return (
    <section className="stats" aria-label={t.statsTelemetry ?? "Telemetry"}>
      <article className="card stat rx">
        <span className="stat-label">↓ {t.statsDownload}</span>
        <strong className="stat-value" id="stat-rx">{formatRate(latest.rxBps)}</strong>
        <Sparkline values={rxHistory} tone="rx" />
        <span className="stat-sub">{formatCount(latest.rxPackets)} {t.statsPackets}</span>
      </article>
      <article className="card stat tx">
        <span className="stat-label">↑ {t.statsUpload}</span>
        <strong className="stat-value" id="stat-tx">{formatRate(latest.txBps)}</strong>
        <Sparkline values={txHistory} tone="tx" />
        <span className="stat-sub">{formatCount(latest.txPackets)} {t.statsPackets}</span>
      </article>
      <article className="card stat wide ping">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <span className="stat-label">⚡ Ping (RTT)</span>
            <strong className="stat-value small" style={{ color: latest.rttMs <= 0 ? undefined : latest.rttMs < 60 ? "#4ade80" : latest.rttMs < 120 ? "#facc15" : "#f87171" }}>
              {latest.rttMs > 0 ? `${latest.rttMs} ms` : "—"}
            </strong>
          </div>
          <div>
            <span className="stat-label">Jitter</span>
            <strong className="stat-value small" style={{ color: latest.jitterMs <= 0 ? undefined : latest.jitterMs < 15 ? "#4ade80" : latest.jitterMs < 40 ? "#facc15" : "#f87171" }}>
              {latest.jitterMs > 0 ? `${latest.jitterMs} ms` : "—"}
            </strong>
          </div>
          <div>
            <span className="stat-label">{t.statsDropped}</span>
            <strong className="stat-value small" id="stat-drop">{formatCount(latest.dropped)}</strong>
          </div>
        </div>
      </article>
    </section>
  );
}

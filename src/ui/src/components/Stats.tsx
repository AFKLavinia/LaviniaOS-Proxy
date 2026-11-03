import { memo, useId } from "react";
import { formatCount, formatRate } from "../lib/format";
import type { Telemetry } from "../ipc";
import type { Translations } from "../lib/i18n";

export const HISTORY = 48;

interface Point {
  x: number;
  y: number;
}

/**
 * Builds a smooth Catmull-Rom cubic Bezier curve path string from data points.
 */
function catmullRomPath(pts: Point[]): { linePath: string; areaPath: string } {
  if (pts.length === 0) return { linePath: "", areaPath: "" };
  if (pts.length === 1) {
    const p = pts[0];
    return { linePath: `M ${p.x} ${p.y}`, areaPath: "" };
  }

  let d = `M ${pts[0].x} ${pts[0].y}`;

  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = i > 0 ? pts[i - 1] : pts[i];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = i < pts.length - 2 ? pts[i + 2] : p2;

    const cp1x = p1.x + (p2.x - p0.x) / 6;
    const cp1y = p1.y + (p2.y - p0.y) / 6;
    const cp2x = p2.x - (p3.x - p1.x) / 6;
    const cp2y = p2.y - (p3.y - p1.y) / 6;

    d += ` C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${p2.x.toFixed(2)} ${p2.y.toFixed(2)}`;
  }

  const first = pts[0];
  const last = pts[pts.length - 1];
  const h = 44;
  const areaPath = `${d} L ${last.x.toFixed(2)} ${h} L ${first.x.toFixed(2)} ${h} Z`;

  return { linePath: d, areaPath };
}

function Sparkline({ values, tone }: { values: number[]; tone: "rx" | "tx" }) {
  const gradId = useId();
  const w = 200;
  const h = 44;

  if (values.length === 0) {
    return (
      <svg className={`spark ${tone}`} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden="true" />
    );
  }

  const max = Math.max(1, ...values);
  const step = w / (HISTORY - 1);
  const pts: Point[] = values.map((v, i) => ({
    x: (i + HISTORY - values.length) * step,
    y: Math.max(2, Math.min(h - 2, h - (v / max) * (h - 6) - 3)),
  }));

  const { linePath, areaPath } = catmullRomPath(pts);
  const strokeColor = tone === "rx" ? "var(--cyan, #4cc9f0)" : "var(--violet, #7c5cff)";
  const stopColor = tone === "rx" ? "#4cc9f0" : "#7c5cff";

  return (
    <svg className={`spark ${tone}`} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden="true">
      <defs>
        <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={stopColor} stopOpacity="0.28" />
          <stop offset="85%" stopColor={stopColor} stopOpacity="0.04" />
          <stop offset="100%" stopColor={stopColor} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={areaPath} fill={`url(#${gradId})`} />
      <path
        d={linePath}
        fill="none"
        stroke={strokeColor}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

interface Props {
  latest: Telemetry;
  rxHistory: number[];
  txHistory: number[];
  t: Translations & { statsTelemetry?: string };
}

export const Stats = memo(function Stats({ latest, rxHistory, txHistory, t }: Props) {
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
            <span className="stat-label">Buffer Debloat</span>
            <strong className="stat-value small" style={{ color: latest.phase === "stopped" ? "var(--muted, #64748b)" : latest.gameMode ? "var(--green, #4ade80)" : "var(--cyan, #38bdf8)" }}>
              {latest.phase === "stopped" ? (t.debloatDisabled ?? "Devre Dışı") : latest.gameMode ? (t.debloatGameMode ?? "Oyun Modu (CoDel)") : (t.debloatActive ?? "Aktif (RFC 8289)")}
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
});

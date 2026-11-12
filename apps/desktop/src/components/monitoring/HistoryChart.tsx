import { buildLines, type Plot } from "../../services/historyState";
import type { HistoryPoint } from "../../types/history";

export interface Series {
  label: string;
  color: string;
  pick: (p: HistoryPoint) => number;
}

const W = 600;
const H = 120;

/**
 * A time-series chart of stored history. Gaps in collection are drawn as breaks, and the time
 * axis covers the whole selected range, so missing data is visible instead of being hidden.
 */
export function HistoryChart({
  title,
  points,
  bucketMs,
  from,
  to,
  series,
  yMax,
  unit,
}: {
  title: string;
  points: HistoryPoint[];
  bucketMs: number;
  from: number;
  to: number;
  series: Series[];
  yMax: number;
  unit: string;
}) {
  const plot: Plot = { width: W, height: H, from, to, yMax };
  const time = (ms: number) =>
    new Date(ms).toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  const fmt = (v: number) =>
    unit === "%" ? `${Math.round(v)}%` : String(Math.round(v * 100) / 100);
  const summary = series
    .map((s) => {
      const values = points.map(s.pick);
      if (values.length === 0) return `${s.label}: no data`;
      const avg = values.reduce((a, b) => a + b, 0) / values.length;
      return `${s.label}: average ${fmt(avg)}, highest ${fmt(Math.max(...values))}`;
    })
    .join("; ");

  return (
    <figure className="rounded-md border border-border bg-surface p-3">
      <figcaption className="mb-2 flex flex-wrap items-baseline justify-between gap-2">
        <span className="text-sm font-medium">{title}</span>
        <span className="flex gap-3 text-xs text-muted">
          {series.map((s) => (
            <span key={s.label} className="flex items-center gap-1">
              <span
                aria-hidden="true"
                className="inline-block h-0.5 w-3"
                style={{ background: s.color }}
              />
              {s.label}
            </span>
          ))}
        </span>
      </figcaption>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`${title}. ${summary}`}
        className="h-32 w-full"
        preserveAspectRatio="none"
      >
        {[0, 0.5, 1].map((f) => (
          <line
            key={f}
            x1={0}
            x2={W}
            y1={H - f * H}
            y2={H - f * H}
            stroke="var(--border)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {series.map((s) => {
          const { lines, dots } = buildLines(points, bucketMs, s.pick, plot);
          return (
            <g key={s.label}>
              {lines.map((l) => (
                <polyline
                  key={l}
                  points={l}
                  fill="none"
                  stroke={s.color}
                  strokeWidth="1.5"
                  strokeLinejoin="round"
                  vectorEffect="non-scaling-stroke"
                />
              ))}
              {dots.map((d) => (
                <circle key={`${d.x}-${d.y}`} cx={d.x} cy={d.y} r="2.5" fill={s.color} />
              ))}
            </g>
          );
        })}
      </svg>
      <div className="mt-1 flex justify-between text-[11px] text-muted">
        <span>{time(from)}</span>
        <span>
          0–{fmt(yMax)}
          {unit === "%" ? "" : ` ${unit}`}
        </span>
        <span>{time(to)}</span>
      </div>
    </figure>
  );
}

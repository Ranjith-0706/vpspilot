import type { HistoryPoint, HistoryRange } from "../types/history";

export const MIN_RETENTION_DAYS = 1;
export const MAX_RETENTION_DAYS = 90;
export const DEFAULT_RETENTION_DAYS = 7;

export interface RangeOption {
  secs: number;
  label: string;
}

const ALL_RANGES: RangeOption[] = [
  { secs: 3600, label: "Last hour" },
  { secs: 6 * 3600, label: "Last 6 hours" },
  { secs: 24 * 3600, label: "Last 24 hours" },
  { secs: 7 * 86400, label: "Last 7 days" },
  { secs: 30 * 86400, label: "Last 30 days" },
];

/** Ranges worth offering: nothing longer than the data is kept for (the hour is always there). */
export function rangesFor(retentionDays: number): RangeOption[] {
  const kept = ALL_RANGES.filter((r) => r.secs <= retentionDays * 86400);
  return kept.length > 0 ? kept : ALL_RANGES.slice(0, 1);
}

/** A longer gap than this between two points means collection stopped for a while. */
export function isGap(prev: HistoryPoint, next: HistoryPoint, bucketMs: number): boolean {
  return next.at - prev.at > bucketMs * 2.5;
}

/** Splits points into runs, so a gap in collection is drawn as a break, not a straight line. */
export function splitAtGaps(points: HistoryPoint[], bucketMs: number): HistoryPoint[][] {
  const runs: HistoryPoint[][] = [];
  for (const p of points) {
    const run = runs[runs.length - 1];
    const last = run?.[run.length - 1];
    if (run && last && !isGap(last, p, bucketMs)) run.push(p);
    else runs.push([p]);
  }
  return runs;
}

export interface Plot {
  width: number;
  height: number;
  from: number;
  to: number;
  yMax: number;
}

/** SVG coordinates for one value; time runs left to right over the whole range. */
export function project(p: Plot, at: number, value: number): { x: number; y: number } {
  const span = Math.max(1, p.to - p.from);
  const x = ((Math.min(p.to, Math.max(p.from, at)) - p.from) / span) * p.width;
  const y = p.height - (Math.min(p.yMax, Math.max(0, value)) / p.yMax) * p.height;
  return { x, y };
}

/** One polyline per run of consecutive points, plus the single points that have no neighbour. */
export function buildLines(
  points: HistoryPoint[],
  bucketMs: number,
  pick: (p: HistoryPoint) => number,
  plot: Plot,
): { lines: string[]; dots: Array<{ x: number; y: number }> } {
  const lines: string[] = [];
  const dots: Array<{ x: number; y: number }> = [];
  for (const run of splitAtGaps(points, bucketMs)) {
    const xy = run.map((p) => project(plot, p.at, pick(p)));
    if (xy.length === 1 && xy[0]) dots.push(xy[0]);
    else lines.push(xy.map((c) => `${c.x.toFixed(1)},${c.y.toFixed(1)}`).join(" "));
  }
  return { lines, dots };
}

/** Upper bound of a load chart: the highest value, rounded up to a tidy number (at least 1). */
export function loadAxisMax(points: HistoryPoint[]): number {
  const max = points.reduce((m, p) => Math.max(m, p.load1, p.load5, p.load15), 0);
  if (max <= 1) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(max));
  return Math.ceil(max / magnitude) * magnitude;
}

/** "5 minutes ago" for the age of a stored sample. Past times only. */
export function formatAge(ms: number, now: number): string {
  const secs = Math.max(0, Math.round((now - ms) / 1000));
  if (secs < 90) return "less than a minute ago";
  const mins = Math.round(secs / 60);
  if (mins < 90) return `${mins} minutes ago`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `${hours} hours ago`;
  return `${Math.round(hours / 24)} days ago`;
}

export type HistoryNote =
  { kind: "none" } | { kind: "empty"; text: string } | { kind: "sparse"; text: string };

/** What to say when a range has little or no data, instead of drawing an empty chart. */
export function historyNote(range: HistoryRange, retentionDays: number): HistoryNote {
  if (range.sampleCount === 0) {
    return {
      kind: "empty",
      text:
        "No history recorded in this time range. Samples are stored while monitoring is running, one a minute, and kept for " +
        `${retentionDays} ${retentionDays === 1 ? "day" : "days"}.`,
    };
  }
  if (range.points.length < 2) {
    return {
      kind: "sparse",
      text: "Only one point has been recorded in this range so far. Keep monitoring running to see a trend.",
    };
  }
  return { kind: "none" };
}

/** Whether a response still belongs to what is selected (guards against out-of-order replies). */
export function isCurrent(
  range: HistoryRange | undefined,
  selection: { serverId: string; secs: number } | undefined,
): range is HistoryRange {
  return (
    range !== undefined &&
    selection !== undefined &&
    range.serverId === selection.serverId &&
    range.to - range.from === selection.secs * 1000
  );
}

/** Validates the retention input, returning the whole number of days or null. */
export function parseRetention(text: string): number | null {
  if (!/^\d+$/.test(text.trim())) return null;
  const n = Number(text.trim());
  return n >= MIN_RETENTION_DAYS && n <= MAX_RETENTION_DAYS ? n : null;
}

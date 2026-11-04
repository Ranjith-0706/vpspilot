import type { Tone } from "../components/tones";
import type { HistoryPoint, Metrics, MonitorEvent, ServerMonitor } from "../types/metrics";

/** About five minutes of history at the default poll interval. */
export const MAX_HISTORY = 60;
export const MONITOR_INTERVAL_SECS = 5;

export function pushHistory(history: HistoryPoint[], point: HistoryPoint): HistoryPoint[] {
  const next = [...history, point];
  return next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next;
}

const EMPTY: ServerMonitor = { state: "connecting", history: [] };

/** Pure reducer: folds one backend event into a server's monitor state. */
export function applyMonitorEvent(
  prev: ServerMonitor | undefined,
  event: MonitorEvent,
): ServerMonitor {
  const base = prev ?? EMPTY;
  if (event.type === "sample") {
    const m = event.metrics;
    return {
      state: "online",
      message: undefined,
      latest: m,
      history: pushHistory(base.history, {
        t: m.sampledAt,
        cpu: m.cpuPercent,
        mem: m.memUsedPercent,
      }),
    };
  }
  // Status: keep the last sample so the UI can show it as stale instead of blanking out.
  return { ...base, state: event.state, message: event.message };
}

/** Same thresholds as the usage bars: under 70% fine, under 85% warning, otherwise critical. */
export function usageTone(percent: number): Tone {
  if (percent >= 85) return "bad";
  if (percent >= 70) return "warn";
  return "ok";
}

export interface LiveSummary {
  monitored: number;
  online: number;
  avgCpu: number | null;
  avgMem: number | null;
  avgDisk: number | null;
}

/** Summarizes servers that are currently online. Averages are null when none are. */
export function summarizeLive(monitors: Array<ServerMonitor | undefined>): LiveSummary {
  const present = monitors.filter(
    (m): m is ServerMonitor => m !== undefined && m.state !== "stopped",
  );
  const live: Metrics[] = present
    .filter((m) => m.state === "online" && m.latest !== undefined)
    .map((m) => m.latest as Metrics);
  const avg = (pick: (m: Metrics) => number) =>
    live.length ? Math.round(live.reduce((n, m) => n + pick(m), 0) / live.length) : null;
  return {
    monitored: present.length,
    online: live.length,
    avgCpu: avg((m) => m.cpuPercent),
    avgMem: avg((m) => m.memUsedPercent),
    avgDisk: avg((m) => m.diskUsedPercent),
  };
}

/** Formats a size given in KiB, e.g. 1048576 -> "1.0 GiB". */
export function formatKib(kib: number): string {
  if (!Number.isFinite(kib) || kib < 0) return "—";
  const units = ["KiB", "MiB", "GiB", "TiB", "PiB"];
  let value = kib;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`;
}

/** Formats seconds as "3d 4h", "5h 12m" or "7m". */
export function formatUptime(secs: number): string {
  if (!Number.isFinite(secs) || secs < 0) return "—";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

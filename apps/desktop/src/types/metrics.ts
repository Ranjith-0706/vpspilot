/** One monitoring sample from a server. Mirrors the Rust `Metrics` struct. */
export interface Metrics {
  cpuPercent: number;
  cores: number;
  load1: number;
  load5: number;
  load15: number;
  memTotalKib: number;
  memAvailableKib: number;
  memUsedPercent: number;
  swapTotalKib: number;
  swapUsedKib: number;
  diskTotalKib: number;
  diskUsedKib: number;
  diskUsedPercent: number;
  uptimeSecs: number;
  /** Unix epoch milliseconds (set by VPSPilot, not the remote clock). */
  sampledAt: number;
}

export type MonitorStatus = "connecting" | "online" | "retrying" | "failed" | "stopped";

/** Events streamed from the Rust monitor. */
export type MonitorEvent =
  { type: "status"; state: MonitorStatus; message?: string } | { type: "sample"; metrics: Metrics };

export interface HistoryPoint {
  t: number;
  cpu: number;
  mem: number;
}

/** What the UI knows about one server's monitor. */
export interface ServerMonitor {
  state: MonitorStatus;
  message?: string | undefined;
  /** Last successful sample. Kept (as stale data) when the monitor retries, fails or stops. */
  latest?: Metrics | undefined;
  history: HistoryPoint[];
}

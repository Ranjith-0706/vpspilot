/** One chart point: the average of the stored samples in a time bucket. Mirrors Rust `HistoryPoint`. */
export interface HistoryPoint {
  /** Unix epoch milliseconds (mean sample time of the bucket). */
  at: number;
  cpuPercent: number;
  memUsedPercent: number;
  diskUsedPercent: number;
  load1: number;
  load5: number;
  load15: number;
  samples: number;
}

/** Stored monitoring history for one server. Historical only: it says nothing about now. */
export interface HistoryRange {
  serverId: string;
  from: number;
  to: number;
  bucketMs: number;
  points: HistoryPoint[];
  sampleCount: number;
  firstSampleAt: number | null;
  lastSampleAt: number | null;
  retentionDays: number;
}

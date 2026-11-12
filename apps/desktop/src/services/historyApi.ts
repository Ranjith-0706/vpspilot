import type { HistoryRange } from "../types/history";
import { call } from "./ipc";

/** Reads what was stored earlier. Nothing here connects to a server. */
export const historyApi = {
  get: (serverId: string, rangeSecs: number, maxPoints?: number) =>
    call<HistoryRange>("get_metric_history", {
      serverId,
      rangeSecs,
      maxPoints: maxPoints ?? null,
    }),
  getRetentionDays: () => call<number>("get_history_retention_days"),
  setRetentionDays: (days: number) => call<number>("set_history_retention_days", { days }),
};

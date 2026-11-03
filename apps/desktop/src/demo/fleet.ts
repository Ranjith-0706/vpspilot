import type { DemoServer } from "../types";

export interface FleetSummary {
  total: number;
  online: number;
  warning: number;
  offline: number;
  /** Servers that are up: online + warning. */
  reachable: number;
  avgCpu: number;
  avgRam: number;
  avgDisk: number;
}

/** Summarizes demo servers. A server with a warning is still up, so it counts as reachable. */
export function summarizeFleet(servers: DemoServer[]): FleetSummary {
  const online = servers.filter((s) => s.status === "online").length;
  const warning = servers.filter((s) => s.status === "warning").length;
  const offline = servers.filter((s) => s.status === "offline").length;
  const up = servers.filter((s) => s.status !== "offline");
  const avg = (pick: (s: DemoServer) => number) =>
    up.length ? Math.round(up.reduce((n, s) => n + pick(s), 0) / up.length) : 0;
  return {
    total: servers.length,
    online,
    warning,
    offline,
    reachable: online + warning,
    avgCpu: avg((s) => s.cpuPercent),
    avgRam: avg((s) => s.ramPercent),
    avgDisk: avg((s) => s.diskPercent),
  };
}

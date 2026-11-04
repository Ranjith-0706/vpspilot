import { describe, expect, it } from "vitest";
import {
  MAX_HISTORY,
  applyMonitorEvent,
  formatKib,
  formatUptime,
  pushHistory,
  summarizeLive,
  usageTone,
} from "./monitorState";
import type { Metrics, ServerMonitor } from "../types/metrics";

const metrics = (patch: Partial<Metrics> = {}): Metrics => ({
  cpuPercent: 10,
  cores: 2,
  load1: 0.1,
  load5: 0.1,
  load15: 0.1,
  memTotalKib: 1000,
  memAvailableKib: 500,
  memUsedPercent: 50,
  swapTotalKib: 0,
  swapUsedKib: 0,
  diskTotalKib: 1000,
  diskUsedKib: 400,
  diskUsedPercent: 40,
  uptimeSecs: 100,
  sampledAt: 1,
  ...patch,
});

describe("applyMonitorEvent", () => {
  it("goes connecting -> online on the first sample", () => {
    let s = applyMonitorEvent(undefined, { type: "status", state: "connecting" });
    expect(s.state).toBe("connecting");
    s = applyMonitorEvent(s, { type: "sample", metrics: metrics({ cpuPercent: 33 }) });
    expect(s.state).toBe("online");
    expect(s.latest?.cpuPercent).toBe(33);
    expect(s.history).toHaveLength(1);
  });

  it("keeps the last sample as stale data when retrying, failing or stopping", () => {
    const online = applyMonitorEvent(undefined, {
      type: "sample",
      metrics: metrics({ cpuPercent: 77 }),
    });
    for (const state of ["retrying", "failed", "stopped"] as const) {
      const s = applyMonitorEvent(online, { type: "status", state, message: "why" });
      expect(s.state).toBe(state);
      expect(s.message).toBe("why");
      expect(s.latest?.cpuPercent).toBe(77);
      expect(s.history).toHaveLength(1);
    }
  });

  it("clears an old message when a sample arrives", () => {
    const retrying = applyMonitorEvent(undefined, {
      type: "status",
      state: "retrying",
      message: "down",
    });
    expect(
      applyMonitorEvent(retrying, { type: "sample", metrics: metrics() }).message,
    ).toBeUndefined();
  });

  it("does not mutate the previous state", () => {
    const a = applyMonitorEvent(undefined, { type: "sample", metrics: metrics() });
    const frozen = JSON.stringify(a);
    applyMonitorEvent(a, { type: "sample", metrics: metrics({ sampledAt: 2 }) });
    expect(JSON.stringify(a)).toBe(frozen);
  });
});

describe("pushHistory", () => {
  it("caps the history length and keeps the newest points", () => {
    let h: ReturnType<typeof pushHistory> = [];
    for (let i = 0; i < MAX_HISTORY + 10; i++) h = pushHistory(h, { t: i, cpu: i, mem: i });
    expect(h).toHaveLength(MAX_HISTORY);
    expect(h[0]?.t).toBe(10);
    expect(h[h.length - 1]?.t).toBe(MAX_HISTORY + 9);
  });
});

describe("usageTone", () => {
  it.each([
    [0, "ok"],
    [69.9, "ok"],
    [70, "warn"],
    [84.9, "warn"],
    [85, "bad"],
    [100, "bad"],
  ] as const)("%s%% is %s", (p, tone) => expect(usageTone(p)).toBe(tone));
});

describe("summarizeLive", () => {
  const online = (patch: Partial<Metrics>): ServerMonitor =>
    applyMonitorEvent(undefined, { type: "sample", metrics: metrics(patch) });

  it("averages only online servers and counts monitored ones", () => {
    const failed = applyMonitorEvent(online({ cpuPercent: 99 }), {
      type: "status",
      state: "failed",
    });
    const s = summarizeLive([
      online({ cpuPercent: 20 }),
      online({ cpuPercent: 40 }),
      failed,
      undefined,
    ]);
    expect(s).toMatchObject({ monitored: 3, online: 2, avgCpu: 30 });
  });

  it("excludes stopped servers entirely and returns null averages when nothing is online", () => {
    const stopped = applyMonitorEvent(online({}), { type: "status", state: "stopped" });
    expect(summarizeLive([stopped, undefined])).toEqual({
      monitored: 0,
      online: 0,
      avgCpu: null,
      avgMem: null,
      avgDisk: null,
    });
  });
});

describe("formatKib", () => {
  it.each([
    [0, "0 KiB"],
    [512, "512 KiB"],
    [1024, "1.0 MiB"],
    [1_048_576, "1.0 GiB"],
    [2_048_000, "2.0 GiB"],
    [150 * 1_048_576, "150 GiB"],
    [-1, "—"],
    [Number.NaN, "—"],
  ])("%s -> %s", (kib, text) => expect(formatKib(kib)).toBe(text));
});

describe("formatUptime", () => {
  it.each([
    [0, "0m"],
    [59, "0m"],
    [3600 * 5 + 60 * 12, "5h 12m"],
    [86400 * 3 + 3600 * 4, "3d 4h"],
    [-5, "—"],
  ])("%s -> %s", (s, text) => expect(formatUptime(s)).toBe(text));
});

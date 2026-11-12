import { describe, expect, it } from "vitest";
import {
  buildLines,
  formatAge,
  historyNote,
  isCurrent,
  isGap,
  loadAxisMax,
  parseRetention,
  project,
  rangesFor,
  splitAtGaps,
  type Plot,
} from "./historyState";
import type { HistoryPoint, HistoryRange } from "../types/history";

const MIN = 60_000;
const BUCKET = MIN;

const point = (at: number, cpu = 10, load = 0.5): HistoryPoint => ({
  at,
  cpuPercent: cpu,
  memUsedPercent: 50,
  diskUsedPercent: 40,
  load1: load,
  load5: load,
  load15: load,
  samples: 1,
});

const range = (patch: Partial<HistoryRange> = {}): HistoryRange => ({
  serverId: "a",
  from: 0,
  to: 3600_000,
  bucketMs: BUCKET,
  points: [point(MIN), point(2 * MIN)],
  sampleCount: 2,
  firstSampleAt: MIN,
  lastSampleAt: 2 * MIN,
  retentionDays: 7,
  ...patch,
});

describe("range options", () => {
  it("offers only ranges the retention can fill", () => {
    expect(rangesFor(7).map((r) => r.label)).toEqual([
      "Last hour",
      "Last 6 hours",
      "Last 24 hours",
      "Last 7 days",
    ]);
    expect(rangesFor(1).map((r) => r.secs)).toEqual([3600, 6 * 3600, 24 * 3600]);
    expect(rangesFor(30).at(-1)?.label).toBe("Last 30 days");
  });

  it("always offers at least the last hour", () => {
    expect(rangesFor(0).map((r) => r.secs)).toEqual([3600]);
  });
});

describe("gaps in collection", () => {
  it("does not join points across a gap", () => {
    const pts = [point(MIN), point(2 * MIN), point(40 * MIN), point(41 * MIN)];
    const runs = splitAtGaps(pts, BUCKET);
    expect(runs.map((r) => r.length)).toEqual([2, 2]);
    expect(isGap(pts[1]!, pts[2]!, BUCKET)).toBe(true);
    expect(isGap(pts[0]!, pts[1]!, BUCKET)).toBe(false);
  });

  it("returns no runs for no points", () => {
    expect(splitAtGaps([], BUCKET)).toEqual([]);
  });

  it("draws one line per run and a dot for a lone point", () => {
    const plot: Plot = { width: 600, height: 100, from: 0, to: 60 * MIN, yMax: 100 };
    const pts = [point(MIN), point(2 * MIN), point(30 * MIN), point(50 * MIN), point(51 * MIN)];
    const { lines, dots } = buildLines(pts, BUCKET, (p) => p.cpuPercent, plot);
    expect(lines).toHaveLength(2);
    expect(dots).toHaveLength(1);
  });
});

describe("chart geometry", () => {
  const plot: Plot = { width: 600, height: 100, from: 1000, to: 2000, yMax: 100 };

  it("maps time left to right and values bottom to top", () => {
    expect(project(plot, 1000, 0)).toEqual({ x: 0, y: 100 });
    expect(project(plot, 2000, 100)).toEqual({ x: 600, y: 0 });
    expect(project(plot, 1500, 50)).toEqual({ x: 300, y: 50 });
  });

  it("clamps values and times that fall outside the plot", () => {
    expect(project(plot, 0, -5)).toEqual({ x: 0, y: 100 });
    expect(project(plot, 9999, 500)).toEqual({ x: 600, y: 0 });
  });

  it("never divides by zero for an empty time span", () => {
    const flat = { ...plot, from: 5, to: 5 };
    expect(Number.isFinite(project(flat, 5, 10).x)).toBe(true);
  });
});

describe("load axis", () => {
  it("is at least 1 and rounds up to a tidy number", () => {
    expect(loadAxisMax([])).toBe(1);
    expect(loadAxisMax([point(1, 0, 0.3)])).toBe(1);
    expect(loadAxisMax([point(1, 0, 1.2)])).toBe(2);
    expect(loadAxisMax([point(1, 0, 3.4)])).toBe(4);
    expect(loadAxisMax([point(1, 0, 12), point(2, 0, 3)])).toBe(20);
  });
});

describe("age of the last sample", () => {
  const now = 10 * 3600_000;
  it("describes how old history is, in the past tense", () => {
    expect(formatAge(now - 20_000, now)).toBe("less than a minute ago");
    expect(formatAge(now - 5 * MIN, now)).toBe("5 minutes ago");
    expect(formatAge(now - 3 * 3600_000, now)).toBe("3 hours ago");
    expect(formatAge(now - 72 * 3600_000, now)).toBe("3 days ago");
  });

  it("treats a timestamp slightly in the future as just now", () => {
    expect(formatAge(now + 5000, now)).toBe("less than a minute ago");
  });
});

describe("missing and sparse data", () => {
  it("says so when nothing was recorded, instead of showing an empty chart as data", () => {
    const n = historyNote(
      range({ points: [], sampleCount: 0, firstSampleAt: null, lastSampleAt: null }),
      7,
    );
    expect(n.kind).toBe("empty");
    expect(n.kind === "empty" && n.text).toContain("7 days");
  });

  it("uses the singular for one day", () => {
    const n = historyNote(range({ points: [], sampleCount: 0 }), 1);
    expect(n.kind === "empty" && n.text).toContain("1 day.");
  });

  it("flags a single point as too little to show a trend", () => {
    expect(historyNote(range({ points: [point(MIN)], sampleCount: 1 }), 7).kind).toBe("sparse");
  });

  it("has nothing to add when there is a trend", () => {
    expect(historyNote(range(), 7).kind).toBe("none");
  });
});

describe("switching servers and ranges", () => {
  it("accepts a reply only for the current server and range", () => {
    const sel = { serverId: "a", secs: 3600 };
    expect(isCurrent(range(), sel)).toBe(true);
    // A late reply for the previously selected server is not shown as the current one.
    expect(isCurrent(range({ serverId: "b" }), sel)).toBe(false);
    // Nor is a reply for a different time range.
    expect(isCurrent(range({ to: 24 * 3600_000 }), sel)).toBe(false);
    expect(isCurrent(undefined, sel)).toBe(false);
    expect(isCurrent(range(), undefined)).toBe(false);
  });

  it("keeps each server's data separate", () => {
    const a = range({ serverId: "a", points: [point(MIN, 10)] });
    const b = range({ serverId: "b", points: [point(MIN, 90)] });
    expect(isCurrent(a, { serverId: "a", secs: 3600 })).toBe(true);
    expect(isCurrent(b, { serverId: "a", secs: 3600 })).toBe(false);
    expect(isCurrent(b, { serverId: "b", secs: 3600 })).toBe(true);
  });
});

describe("retention input", () => {
  it("accepts whole days from 1 to 90", () => {
    expect(parseRetention("7")).toBe(7);
    expect(parseRetention(" 30 ")).toBe(30);
    expect(parseRetention("1")).toBe(1);
    expect(parseRetention("90")).toBe(90);
  });

  it("rejects everything else", () => {
    for (const bad of ["", "0", "91", "-3", "1.5", "7d", "abc", "1e2", "0x10"]) {
      expect(parseRetention(bad), bad).toBeNull();
    }
  });
});

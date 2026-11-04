import { describe, expect, it } from "vitest";
import {
  DEFAULT_TAIL,
  TAIL_OPTIONS,
  countLevels,
  filterLogLines,
  formatLogTime,
  guessLevel,
} from "./logsState";
import type { LogLine } from "../types/docker";

const line = (message: string, time?: string): LogLine => (time ? { time, message } : { message });

describe("guessLevel", () => {
  it.each([
    ["ERROR failed to reach db", "error"],
    ["level=error msg=boom", "error"],
    ['{"level":"fatal","msg":"x"}', "error"],
    ["panic: runtime error", "error"],
    ["Traceback (most recent call last):", "error"],
    ["WARN disk almost full", "warn"],
    ["level=warning msg=slow", "warn"],
    ["DEBUG cache miss", "debug"],
    ["TRACE entering handler", "debug"],
    ["INFO server listening", "info"],
    ["notice: reloading", "info"],
  ] as const)("reads %j as %s", (text, want) => {
    expect(guessLevel(text)).toBe(want);
  });

  it("returns null when there is no level word", () => {
    expect(guessLevel("GET /index.html 200 12ms")).toBeNull();
    expect(guessLevel("")).toBeNull();
  });

  it("matches whole words only", () => {
    expect(guessLevel("terrorist informs the debugger")).toBeNull();
    expect(guessLevel("warned about errors")).toBeNull();
    expect(guessLevel("interrupt")).toBeNull();
  });

  it("prefers the most severe level present", () => {
    expect(guessLevel("info: error while retrying (warn)")).toBe("error");
    expect(guessLevel("debug: warn threshold")).toBe("warn");
  });
});

describe("filterLogLines", () => {
  const lines = [
    line("INFO server listening on :8080"),
    line("ERROR failed to reach db"),
    line("WARN slow response"),
    line("GET /health 200"),
    line("ERROR timeout talking to cache"),
  ];

  it("returns the same array when nothing is filtered", () => {
    expect(filterLogLines(lines, "", "all")).toBe(lines);
    expect(filterLogLines(lines, "   ", "all")).toBe(lines);
  });

  it("filters by level", () => {
    expect(filterLogLines(lines, "", "error")).toHaveLength(2);
    expect(filterLogLines(lines, "", "warn")).toHaveLength(1);
    expect(filterLogLines(lines, "", "debug")).toHaveLength(0);
  });

  it("filters by text without regard to case", () => {
    expect(filterLogLines(lines, "DB", "all").map((l) => l.message)).toEqual([
      "ERROR failed to reach db",
    ]);
  });

  it("combines level and text", () => {
    expect(filterLogLines(lines, "cache", "error")).toHaveLength(1);
    expect(filterLogLines(lines, "cache", "warn")).toHaveLength(0);
  });

  it("treats the query as plain text, not a pattern", () => {
    expect(filterLogLines(lines, ".*", "all")).toEqual([]);
    expect(filterLogLines(lines, "[", "all")).toEqual([]);
    expect(filterLogLines([line("a.b (c)")], "(c)", "all")).toHaveLength(1);
  });

  it("does not change its input", () => {
    const copy = [...lines];
    filterLogLines(lines, "error", "all");
    expect(lines).toEqual(copy);
  });
});

describe("countLevels", () => {
  it("counts guessed errors and warnings", () => {
    expect(
      countLevels([
        line("ERROR a"),
        line("WARN b"),
        line("error c"),
        line("plain"),
        line("INFO d"),
      ]),
    ).toEqual({ error: 2, warn: 1 });
    expect(countLevels([])).toEqual({ error: 0, warn: 0 });
  });
});

describe("formatLogTime", () => {
  it("formats a docker timestamp with milliseconds in local time", () => {
    const out = formatLogTime("2026-11-04T10:15:30.123456789Z");
    expect(out).toMatch(/^\d{2}:\d{2}:\d{2}\.\d{3}$/);
    const d = new Date("2026-11-04T10:15:30.123Z");
    expect(out).toBe(
      `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:30.123`,
    );
  });

  it("returns an empty string when there is no usable timestamp", () => {
    expect(formatLogTime(undefined)).toBe("");
    expect(formatLogTime("not a time")).toBe("");
    expect(formatLogTime("")).toBe("");
  });
});

describe("tail options", () => {
  it("stay within the backend's 1 to 1000 limit and include the default", () => {
    for (const n of TAIL_OPTIONS) {
      expect(n).toBeGreaterThanOrEqual(1);
      expect(n).toBeLessThanOrEqual(1000);
    }
    expect(TAIL_OPTIONS).toContain(DEFAULT_TAIL);
  });
});

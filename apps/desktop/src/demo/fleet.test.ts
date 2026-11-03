import { describe, expect, it } from "vitest";
import { mockServers } from "./demoData";
import { summarizeFleet } from "./fleet";

describe("summarizeFleet", () => {
  it("counts warning servers as up, so the totals are consistent", () => {
    const s = summarizeFleet(mockServers);
    expect(s.total).toBe(4);
    expect(s.online).toBe(2);
    expect(s.warning).toBe(1);
    expect(s.offline).toBe(1);
    expect(s.reachable).toBe(3);
    expect(s.online + s.warning + s.offline).toBe(s.total);
  });

  it("averages only servers that are up", () => {
    const s = summarizeFleet(mockServers);
    expect(s.avgCpu).toBe(Math.round((34 + 78 + 12) / 3));
  });

  it("handles an empty fleet", () => {
    expect(summarizeFleet([])).toMatchObject({ total: 0, reachable: 0, avgCpu: 0 });
  });
});

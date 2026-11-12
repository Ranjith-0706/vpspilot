import { describe, expect, it } from "vitest";
import {
  RECOMMENDED_RULES,
  applyAlertEvent,
  describeCondition,
  describeEvent,
  dropServerAlerts,
  formatWindow,
  initialRuleValues,
  isAlertStale,
  staleAlertNote,
  severityOf,
  validateRuleForm,
} from "./alertsState";
import { applyMonitorEvent } from "./monitorState";
import type { ActiveAlert, AlertEvent, RuleFormValues } from "../types/alerts";
import type { Metrics, MonitorEvent, ServerMonitor } from "../types/metrics";

const firing = (over: Partial<AlertEvent> = {}): AlertEvent => ({
  id: 1,
  ruleId: "r1",
  serverId: "s1",
  metric: "cpu",
  threshold: 90,
  state: "firing",
  value: 96.4,
  at: 1000,
  ...over,
});

describe("applyAlertEvent", () => {
  it("adds a firing alert", () => {
    const next = applyAlertEvent([], firing());
    expect(next).toEqual([
      { ruleId: "r1", serverId: "s1", metric: "cpu", threshold: 90, since: 1000, value: 96.4 },
    ]);
  });

  it("removes an alert when it resolves", () => {
    const active = applyAlertEvent([], firing());
    expect(applyAlertEvent(active, firing({ state: "resolved", value: 50, at: 2000 }))).toEqual([]);
  });

  it("resolves only the matching rule and server", () => {
    let active = applyAlertEvent([], firing());
    active = applyAlertEvent(active, firing({ serverId: "s2", at: 1500 }));
    active = applyAlertEvent(active, firing({ state: "resolved", value: null, at: 2000 }));
    expect(active.map((a) => a.serverId)).toEqual(["s2"]);
  });

  it("does not duplicate a repeated firing event", () => {
    let active = applyAlertEvent([], firing());
    active = applyAlertEvent(active, firing({ value: 99, at: 1100 }));
    expect(active).toHaveLength(1);
    expect(active[0]?.value).toBe(99);
  });

  it("keeps the list ordered by start time", () => {
    let active = applyAlertEvent([], firing({ ruleId: "b", at: 2000 }));
    active = applyAlertEvent(active, firing({ ruleId: "a", at: 1000 }));
    expect(active.map((a) => a.ruleId)).toEqual(["a", "b"]);
  });

  it("ignores events without a rule id", () => {
    const active: ActiveAlert[] = [];
    expect(applyAlertEvent(active, firing({ ruleId: null }))).toBe(active);
  });
});

describe("dropServerAlerts", () => {
  it("removes every alert for that server only", () => {
    let active = applyAlertEvent([], firing());
    active = applyAlertEvent(active, firing({ serverId: "s2", at: 1500 }));
    expect(dropServerAlerts(active, "s1").map((a) => a.serverId)).toEqual(["s2"]);
  });

  it("returns the same array when nothing changes", () => {
    const active = applyAlertEvent([], firing());
    expect(dropServerAlerts(active, "other")).toBe(active);
  });
});

describe("formatting", () => {
  it("formats the time window", () => {
    expect(formatWindow(1, 5)).toBe("5 s");
    expect(formatWindow(3, 5)).toBe("15 s");
    expect(formatWindow(12, 5)).toBe("1 min");
    expect(formatWindow(18, 5)).toBe("1 min 30 s");
  });

  it("describes a rule", () => {
    expect(describeCondition({ metric: "disk", threshold: 90, consecutive: 1 })).toBe(
      "Disk at or above 90%",
    );
    expect(describeCondition({ metric: "cpu", threshold: 80, consecutive: 3 })).toBe(
      "CPU at or above 80% for 3 checks in a row (about 15 s)",
    );
  });

  it("describes events", () => {
    expect(describeEvent(firing())).toBe("CPU reached 96% (limit 90%)");
    expect(describeEvent(firing({ state: "resolved", value: 41.2 }))).toBe("CPU back down to 41%");
    expect(describeEvent(firing({ state: "resolved", value: null }))).toMatch(/ended/);
  });

  it("uses a stronger tone for high thresholds", () => {
    expect(severityOf(95)).toBe("bad");
    expect(severityOf(90)).toBe("bad");
    expect(severityOf(80)).toBe("warn");
  });
});

describe("validateRuleForm", () => {
  const base: RuleFormValues = {
    serverId: "",
    metric: "memory",
    threshold: "85",
    consecutive: "2",
    enabled: true,
  };

  it("builds an input, with an empty server meaning all servers", () => {
    const r = validateRuleForm(base);
    expect(r.errors).toEqual({});
    expect(r.input).toEqual({
      serverId: null,
      metric: "memory",
      threshold: 85,
      consecutive: 2,
      enabled: true,
    });
  });

  it("keeps a chosen server", () => {
    expect(validateRuleForm({ ...base, serverId: "abc" }).input?.serverId).toBe("abc");
  });

  it("accepts the bounds and trims spaces", () => {
    expect(validateRuleForm({ ...base, threshold: " 1 ", consecutive: "60" }).input).toBeDefined();
    expect(validateRuleForm({ ...base, threshold: "100" }).input).toBeDefined();
  });

  it.each(["0", "101", "", "abc", "-5", "1.5", "9e1", "99999"])(
    "rejects threshold %j",
    (threshold) => {
      const r = validateRuleForm({ ...base, threshold });
      expect(r.input).toBeUndefined();
      expect(r.errors.threshold).toBeDefined();
    },
  );

  it.each(["0", "61", "", "x", "2.5"])("rejects consecutive %j", (consecutive) => {
    const r = validateRuleForm({ ...base, consecutive });
    expect(r.input).toBeUndefined();
    expect(r.errors.consecutive).toBeDefined();
  });

  it("reports both errors at once", () => {
    const r = validateRuleForm({ ...base, threshold: "0", consecutive: "0" });
    expect(Object.keys(r.errors).sort()).toEqual(["consecutive", "threshold"]);
  });
});

describe("defaults", () => {
  it("starts a new rule at CPU 90% for 3 checks, on all servers", () => {
    expect(initialRuleValues()).toEqual({
      serverId: "",
      metric: "cpu",
      threshold: "90",
      consecutive: "3",
      enabled: true,
    });
  });

  it("recommended rules are valid and cover each metric once", () => {
    expect(RECOMMENDED_RULES.map((r) => r.metric).sort()).toEqual(["cpu", "disk", "memory"]);
    for (const r of RECOMMENDED_RULES) {
      const form = {
        serverId: "",
        metric: r.metric,
        threshold: String(r.threshold),
        consecutive: String(r.consecutive),
        enabled: r.enabled,
      };
      expect(validateRuleForm(form).input).toEqual(r);
    }
  });
});

describe("alerts during a server outage", () => {
  const sample = (cpuPercent: number): MonitorEvent => ({
    type: "sample",
    metrics: { cpuPercent, sampledAt: 1 } as Metrics,
  });
  const status = (state: "connecting" | "retrying"): MonitorEvent => ({ type: "status", state });
  const play = (events: MonitorEvent[]) =>
    events.reduce<ServerMonitor | undefined>((m, e) => applyMonitorEvent(m, e), undefined);

  it("is confirmed only while fresh samples arrive", () => {
    expect(isAlertStale(play([sample(95)]))).toBe(false);
  });

  it("goes unknown on disconnect and while retrying, without being resolved", () => {
    const alerts = applyAlertEvent([], firing());
    const retrying = play([sample(95), status("retrying")]);
    expect(isAlertStale(retrying)).toBe(true);
    // The monitor reducer never touches alerts; the alert is still listed.
    expect(alerts).toHaveLength(1);
    expect(staleAlertNote(retrying)).toMatch(/not responding/i);
  });

  it("stays unknown while reconnecting and is confirmed again by the first new sample", () => {
    const base = [sample(95), status("retrying"), status("connecting")];
    expect(isAlertStale(play(base))).toBe(true);
    expect(isAlertStale(play([...base, sample(96)]))).toBe(false);
  });

  it("is unknown when nothing is monitoring the server (e.g. after a window reload)", () => {
    expect(isAlertStale(undefined)).toBe(true);
    expect(staleAlertNote(undefined)).toMatch(/not being monitored/i);
  });

  it("recovers: a resolved event after reconnecting clears the alert", () => {
    let alerts = applyAlertEvent([], firing());
    alerts = applyAlertEvent(alerts, firing({ id: 2, state: "resolved", value: 12, at: 2000 }));
    expect(alerts).toHaveLength(0);
  });
});

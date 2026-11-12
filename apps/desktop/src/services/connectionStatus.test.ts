import { describe, expect, it } from "vitest";
import { deriveConnectionStatus, type TerminalSessionState } from "./connectionStatus";
import { applyMonitorEvent } from "./monitorState";
import type { Metrics, MonitorEvent, ServerMonitor } from "../types/metrics";

const metrics: Metrics = {
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
  diskUsedKib: 100,
  diskUsedPercent: 10,
  uptimeSecs: 100,
  sampledAt: 1,
};

const status = (
  state: "connecting" | "online" | "retrying" | "failed" | "stopped",
  extra: { message?: string; cause?: "auth_failed" | "unreachable" | "host_key_changed" } = {},
): MonitorEvent => ({ type: "status", state, ...extra });
const sample: MonitorEvent = { type: "sample", metrics };

/** Folds events the way MonitoringProvider does. */
const fold = (...events: MonitorEvent[]): ServerMonitor | undefined =>
  events.reduce<ServerMonitor | undefined>((m, e) => applyMonitorEvent(m, e), undefined);

const kind = (m: ServerMonitor | undefined, t?: TerminalSessionState) =>
  deriveConnectionStatus(m, t).kind;

describe("connection status model", () => {
  it("1. live monitoring is shown as active", () => {
    const m = fold(status("connecting"), status("online"), sample);
    const s = deriveConnectionStatus(m, undefined);
    expect(s.kind).toBe("monitoring");
    expect(s.monitoring).toBe(true);
    expect(s.tone).toBe("ok");
  });

  it("is not connected when nothing is running", () => {
    const s = deriveConnectionStatus(undefined, undefined);
    expect(s.kind).toBe("not_connected");
    expect(s.monitoring).toBe(false);
    expect(s.terminalOpen).toBe(false);
  });

  it("2. stopping monitoring leaves no active status (stale samples do not count)", () => {
    const m = fold(status("online"), sample, status("stopped", { message: "Monitoring stopped." }));
    expect(m?.latest).toBeDefined(); // the last reading is kept for display only
    const s = deriveConnectionStatus(m, undefined);
    expect(s.kind).toBe("not_connected");
    expect(s.monitoring).toBe(false);
  });

  it("3. an open terminal without monitoring is an active SSH session", () => {
    const s = deriveConnectionStatus(undefined, { kind: "connected" });
    expect(s.kind).toBe("connected");
    expect(s.terminalOpen).toBe(true);
    expect(s.monitoring).toBe(false);
  });

  it("a terminal that is connecting, closed or failed is not a connected session", () => {
    expect(kind(undefined, { kind: "connecting" })).toBe("connecting");
    expect(kind(undefined, { kind: "closed" })).toBe("not_connected");
    expect(kind(undefined, { kind: "error", message: "refused" })).toBe("error");
  });

  it("monitoring wins over a failed terminal, and a terminal wins over a monitor retry", () => {
    expect(kind(fold(status("online")), { kind: "error", message: "x" })).toBe("monitoring");
    expect(kind(fold(status("retrying")), { kind: "connected" })).toBe("connected");
  });

  it("4. a passed connection test cannot make a server connected", () => {
    // The model takes no test input at all, so a result can never change the answer.
    expect(deriveConnectionStatus.length).toBe(2);
    expect(kind(undefined, undefined)).toBe("not_connected");
  });

  it("5. a network drop moves monitoring to reconnecting, then not connected once stopped", () => {
    const online = fold(status("online"), sample);
    expect(kind(online)).toBe("monitoring");
    const dropped = applyMonitorEvent(
      online,
      status("retrying", { message: "Could not reach host", cause: "unreachable" }),
    );
    const s = deriveConnectionStatus(dropped, undefined);
    expect(s.kind).toBe("reconnecting");
    expect(s.monitoring).toBe(false);
    expect(s.detail).toBe("Could not reach host");
    expect(kind(applyMonitorEvent(dropped, status("stopped")))).toBe("not_connected");
  });

  it("6. a reconnect brings monitoring back", () => {
    const m = fold(
      status("online"),
      sample,
      status("retrying"),
      status("connecting"),
      status("online"),
      sample,
    );
    expect(kind(m)).toBe("monitoring");
    expect(m?.message).toBeUndefined();
  });

  it("shows connecting while a monitor starts", () => {
    expect(kind(fold(status("connecting")))).toBe("connecting");
  });

  it("7. an authentication failure is never online", () => {
    const m = fold(
      status("connecting"),
      status("failed", {
        message: "The server rejected the username or credential.",
        cause: "auth_failed",
      }),
    );
    const s = deriveConnectionStatus(m, undefined);
    expect(s.kind).toBe("auth_failed");
    expect(s.tone).toBe("bad");
    expect(s.monitoring).toBe(false);
    expect(s.detail).toContain("rejected");
  });

  it("an authentication failure after earlier samples is still not online", () => {
    const m = fold(status("online"), sample, status("failed", { cause: "auth_failed" }));
    expect(kind(m)).toBe("auth_failed");
  });

  it("other failures are a connection error, not an authentication failure", () => {
    expect(kind(fold(status("failed", { cause: "host_key_changed" })))).toBe("error");
    expect(kind(fold(status("failed", { message: "Too much output" })))).toBe("error");
  });

  it("a new successful event clears the failure cause", () => {
    const m = fold(status("failed", { cause: "auth_failed" }), status("connecting"), sample);
    expect(m?.cause).toBeUndefined();
    expect(kind(m)).toBe("monitoring");
  });

  it("8. the same state gives the same answer wherever it is read (navigation)", () => {
    // Dashboard and Servers both call this with the provider's state, which lives above the router.
    const m = fold(status("online"), sample);
    const t: TerminalSessionState = { kind: "connected" };
    expect(deriveConnectionStatus(m, t)).toEqual(deriveConnectionStatus(m, t));
    // Re-reading after other servers changed does not alter this server's result.
    const other = fold(status("failed", { cause: "auth_failed" }));
    expect(kind(other)).toBe("auth_failed");
    expect(kind(m)).toBe("monitoring");
  });

  it("9. each server keeps its own status", () => {
    const monitors: Record<string, ServerMonitor> = {
      a: fold(status("online"), sample)!,
      b: fold(status("failed", { cause: "auth_failed" }))!,
      c: fold(status("retrying"))!,
      d: fold(status("stopped"))!,
    };
    const terminals: Record<string, TerminalSessionState> = { e: { kind: "connected" } };
    const result = Object.fromEntries(
      ["a", "b", "c", "d", "e", "f"].map((id) => [
        id,
        deriveConnectionStatus(monitors[id], terminals[id]).kind,
      ]),
    );
    expect(result).toEqual({
      a: "monitoring",
      b: "auth_failed",
      c: "reconnecting",
      d: "not_connected",
      e: "connected",
      f: "not_connected",
    });
  });
});

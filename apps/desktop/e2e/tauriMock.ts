import type { Page } from "@playwright/test";
import type { HostKeyRecord, ServerProfile } from "../src/types/servers";
import type { MonitorEvent } from "../src/types/metrics";

/** One stored sample, positioned by how many minutes before "now" it was taken. */
export interface FakeSample {
  agoMin: number;
  cpu: number;
  mem?: number;
  disk?: number;
  load?: number;
}

/** Everything the fake backend knows. Must be JSON-serializable. */
export interface Scenario {
  servers: ServerProfile[];
  hostKeys: HostKeyRecord[];
  /** Stored history per server id. A missing entry means no samples. */
  history?: Record<string, FakeSample[]>;
  /** Delay before a server's history reply, to simulate a slow disk. */
  historyDelayMs?: Record<string, number>;
  /** Make every history query fail with this message. */
  historyError?: string;
  retentionDays?: number;
}

export interface MockCall {
  cmd: string;
  args: Record<string, unknown>;
}

declare global {
  interface Window {
    __MOCK__: {
      calls: MockCall[];
      unmocked: string[];
      emit: (serverId: string, event: unknown) => void;
    };
  }
}

/** The fake `window.__TAURI_INTERNALS__`. Runs inside the page, so it must be self-contained. */
function installInPage(scenario: Scenario) {
  const callbacks = new Map<number, (message: unknown) => void>();
  let nextCallbackId = 1;
  const channels: Record<string, { id: number; index: number }> = {};
  let retention = scenario.retentionDays ?? 7;
  const calls: MockCall[] = [];
  const unmocked: string[] = [];

  const fail = (code: string, message: string) => Promise.reject({ code, message });

  const range = (serverId: string, rangeSecs: number) => {
    const now = Date.now();
    const to = now;
    const from = to - rangeSecs * 1000;
    const samples = (scenario.history?.[serverId] ?? [])
      .map((s) => ({ ...s, at: now - s.agoMin * 60_000 }))
      .filter((s) => s.at >= from && s.at < to)
      .sort((a, b) => a.at - b.at);
    return {
      serverId,
      from,
      to,
      bucketMs: 60_000,
      points: samples.map((s) => ({
        at: s.at,
        cpuPercent: s.cpu,
        memUsedPercent: s.mem ?? 50,
        diskUsedPercent: s.disk ?? 40,
        load1: s.load ?? 0.5,
        load5: s.load ?? 0.4,
        load15: s.load ?? 0.3,
        samples: 1,
      })),
      sampleCount: samples.length,
      firstSampleAt: samples[0]?.at ?? null,
      lastSampleAt: samples[samples.length - 1]?.at ?? null,
      retentionDays: retention,
    };
  };

  const send = (serverId: string, message: unknown) => {
    const ch = channels[serverId];
    if (!ch) return;
    callbacks.get(ch.id)?.({ index: ch.index++, message });
  };

  const invoke = async (cmd: string, args: Record<string, unknown> = {}) => {
    calls.push({
      cmd,
      args: JSON.parse(JSON.stringify(args, (_k, v) => (v?.id ? "[channel]" : v))),
    });
    switch (cmd) {
      case "get_app_info":
        return { name: "VPSPilot", version: "0.0.0-test" };
      case "list_servers":
        return scenario.servers;
      case "list_host_keys":
        return scenario.hostKeys;
      case "list_active_alerts":
      case "list_alert_rules":
      case "list_alert_events":
        return [];
      case "get_history_retention_days":
        return retention;
      case "set_history_retention_days": {
        const days = args.days as number;
        if (!Number.isInteger(days) || days < 1 || days > 90) {
          return fail("validation", "History can be kept for 1 to 90 days.");
        }
        retention = days;
        return days;
      }
      case "get_metric_history": {
        if (scenario.historyError) return fail("database", scenario.historyError);
        const id = args.serverId as string;
        const delay = scenario.historyDelayMs?.[id] ?? 0;
        if (delay) await new Promise((r) => setTimeout(r, delay));
        return range(id, args.rangeSecs as number);
      }
      case "start_monitoring": {
        const ch = args.onEvent as { id: number };
        channels[args.id as string] = { id: ch.id, index: 0 };
        return;
      }
      case "stop_monitoring": {
        // The real backend ends a monitor with a "stopped" status event.
        send(args.id as string, {
          type: "status",
          state: "stopped",
          message: "Monitoring stopped.",
        });
        return;
      }
      case "test_server_connection":
        return {
          outcome: "success",
          message: "Host key verified and authentication succeeded.",
          latencyMs: 12,
          testedAt: Date.now(),
        };
      default:
        unmocked.push(cmd);
        return fail("internal", `Unmocked command: ${cmd}`);
    }
  };

  const w = window as unknown as Record<string, unknown>;
  w.isTauri = true;
  w.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: (message: unknown) => void) => {
      const id = nextCallbackId++;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
  };
  w.__MOCK__ = { calls, unmocked, emit: send };
}

/** Starts the page with a fake Tauri backend. Call before `page.goto`. */
export async function installTauriMock(page: Page, scenario: Scenario): Promise<void> {
  await page.addInitScript(installInPage, scenario);
}

/** Pushes a monitor event into the stream the app opened for `serverId`. */
export async function emitMonitor(page: Page, serverId: string, event: MonitorEvent) {
  await page.evaluate(([id, e]) => window.__MOCK__.emit(id as string, e), [
    serverId,
    event,
  ] as const);
}

export const callsTo = (page: Page, cmd: string): Promise<MockCall[]> =>
  page.evaluate((c) => window.__MOCK__.calls.filter((x) => x.cmd === c), cmd);

export const unmockedCommands = (page: Page): Promise<string[]> =>
  page.evaluate(() => window.__MOCK__.unmocked);

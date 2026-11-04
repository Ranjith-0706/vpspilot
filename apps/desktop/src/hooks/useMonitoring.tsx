import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { startMonitoring, stopMonitoring } from "../services/monitorApi";
import { toAppError } from "../services/ipc";
import { MONITOR_INTERVAL_SECS, applyMonitorEvent } from "../services/monitorState";
import type { MonitorEvent, ServerMonitor } from "../types/metrics";

interface MonitoringContextValue {
  /** Monitor state by server id. Absent means "not monitored". */
  monitors: Record<string, ServerMonitor>;
  start: (serverId: string) => Promise<void>;
  stop: (serverId: string) => Promise<void>;
}

const MonitoringContext = createContext<MonitoringContextValue | null>(null);

/**
 * Holds monitoring state above the router so it survives navigating between pages.
 * Monitors live in the Rust backend and end when the app closes.
 */
export function MonitoringProvider({ children }: { children: ReactNode }) {
  const [monitors, setMonitors] = useState<Record<string, ServerMonitor>>({});
  // Only events from the most recent start() of a server are applied.
  const generation = useRef<Record<string, number>>({});

  const apply = useCallback((serverId: string, gen: number, event: MonitorEvent) => {
    if (generation.current[serverId] !== gen) return;
    setMonitors((prev) => ({ ...prev, [serverId]: applyMonitorEvent(prev[serverId], event) }));
  }, []);

  const start = useCallback(
    async (serverId: string) => {
      const gen = (generation.current[serverId] ?? 0) + 1;
      generation.current[serverId] = gen;
      setMonitors((prev) => ({ ...prev, [serverId]: { state: "connecting", history: [] } }));
      try {
        await startMonitoring(serverId, MONITOR_INTERVAL_SECS, (e) => apply(serverId, gen, e));
      } catch (err) {
        apply(serverId, gen, { type: "status", state: "failed", message: toAppError(err).message });
      }
    },
    [apply],
  );

  const stop = useCallback(async (serverId: string) => {
    try {
      await stopMonitoring(serverId);
    } catch (err) {
      setMonitors((prev) => ({
        ...prev,
        [serverId]: applyMonitorEvent(prev[serverId], {
          type: "status",
          state: "failed",
          message: toAppError(err).message,
        }),
      }));
    }
  }, []);

  const value = useMemo(() => ({ monitors, start, stop }), [monitors, start, stop]);
  return <MonitoringContext.Provider value={value}>{children}</MonitoringContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useMonitoring(): MonitoringContextValue {
  const ctx = useContext(MonitoringContext);
  if (!ctx) throw new Error("useMonitoring must be used inside <MonitoringProvider>");
  return ctx;
}

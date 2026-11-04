import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { startMonitoring, stopMonitoring } from "../services/monitorApi";
import { alertsApi } from "../services/alertsApi";
import { applyAlertEvent, dropServerAlerts } from "../services/alertsState";
import { toAppError } from "../services/ipc";
import { MONITOR_INTERVAL_SECS, applyMonitorEvent } from "../services/monitorState";
import type { ActiveAlert } from "../types/alerts";
import type { MonitorEvent, ServerMonitor, StreamEvent } from "../types/metrics";

interface MonitoringContextValue {
  /** Monitor state by server id. Absent means "not monitored". */
  monitors: Record<string, ServerMonitor>;
  start: (serverId: string) => Promise<void>;
  stop: (serverId: string) => Promise<void>;
  /** Alerts firing right now, oldest first. */
  activeAlerts: ActiveAlert[];
  /** Increases whenever an alert fires or resolves, so history views know to reload. */
  alertVersion: number;
}

const MonitoringContext = createContext<MonitoringContextValue | null>(null);

/**
 * Holds monitoring state above the router so it survives navigating between pages.
 * Monitors live in the Rust backend and end when the app closes.
 */
export function MonitoringProvider({ children }: { children: ReactNode }) {
  const [monitors, setMonitors] = useState<Record<string, ServerMonitor>>({});
  const [activeAlerts, setActiveAlerts] = useState<ActiveAlert[]>([]);
  const [alertVersion, setAlertVersion] = useState(0);
  // Only events from the most recent start() of a server are applied.
  const generation = useRef<Record<string, number>>({});

  // Pick up anything already firing (for example after the window reloads).
  useEffect(() => {
    let active = true;
    alertsApi
      .listActive()
      .then((alerts) => active && setActiveAlerts((prev) => (prev.length > 0 ? prev : alerts)))
      .catch(() => undefined); // not in the desktop app, or the database is unavailable
    return () => {
      active = false;
    };
  }, []);

  const applyStatus = useCallback((serverId: string, event: MonitorEvent) => {
    setMonitors((prev) => ({ ...prev, [serverId]: applyMonitorEvent(prev[serverId], event) }));
    // A monitor that has ended is no longer watching anything, so nothing is firing for it.
    if (event.type === "status" && (event.state === "failed" || event.state === "stopped")) {
      setActiveAlerts((prev) => dropServerAlerts(prev, serverId));
    }
  }, []);

  const apply = useCallback(
    (serverId: string, gen: number, event: StreamEvent) => {
      if (generation.current[serverId] !== gen) return;
      if (event.type === "alert") {
        setActiveAlerts((prev) => applyAlertEvent(prev, event.event));
        setAlertVersion((v) => v + 1);
        return;
      }
      applyStatus(serverId, event);
    },
    [applyStatus],
  );

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

  const stop = useCallback(
    async (serverId: string) => {
      try {
        await stopMonitoring(serverId);
      } catch (err) {
        applyStatus(serverId, {
          type: "status",
          state: "failed",
          message: toAppError(err).message,
        });
      }
    },
    [applyStatus],
  );

  const value = useMemo(
    () => ({ monitors, start, stop, activeAlerts, alertVersion }),
    [monitors, start, stop, activeAlerts, alertVersion],
  );
  return <MonitoringContext.Provider value={value}>{children}</MonitoringContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useMonitoring(): MonitoringContextValue {
  const ctx = useContext(MonitoringContext);
  if (!ctx) throw new Error("useMonitoring must be used inside <MonitoringProvider>");
  return ctx;
}

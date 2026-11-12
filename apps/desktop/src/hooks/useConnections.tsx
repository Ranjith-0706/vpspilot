import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import {
  deriveConnectionStatus,
  type ConnectionStatus,
  type TerminalSessionState,
} from "../services/connectionStatus";
import { useMonitoring } from "./useMonitoring";
import type { ConnectionTestResult } from "../types/servers";

interface ConnectionsContextValue {
  /** Status of one server, from the same model on every page. */
  statusOf: (serverId: string) => ConnectionStatus;
  /** Terminal views report their session here; `undefined` means no session. */
  setTerminal: (serverId: string, state: TerminalSessionState | undefined) => void;
  /** Last connection test per server. Informational only: it never affects `statusOf`. */
  tests: Record<string, ConnectionTestResult>;
  setTest: (serverId: string, result: ConnectionTestResult | undefined) => void;
}

const ConnectionsContext = createContext<ConnectionsContextValue | null>(null);

/**
 * Holds terminal state and test results above the router so every page sees the same thing
 * and nothing is lost when navigating. Monitoring state comes from the monitoring provider.
 * This only records what other parts already know; it opens no connections of its own.
 */
export function ConnectionsProvider({ children }: { children: ReactNode }) {
  const { monitors } = useMonitoring();
  const [terminals, setTerminals] = useState<Record<string, TerminalSessionState>>({});
  const [tests, setTests] = useState<Record<string, ConnectionTestResult>>({});

  const setTerminal = useCallback((serverId: string, state: TerminalSessionState | undefined) => {
    setTerminals((prev) => {
      const next = { ...prev };
      if (state) next[serverId] = state;
      else delete next[serverId];
      return next;
    });
  }, []);

  const setTest = useCallback((serverId: string, result: ConnectionTestResult | undefined) => {
    setTests((prev) => {
      const next = { ...prev };
      if (result) next[serverId] = result;
      else delete next[serverId];
      return next;
    });
  }, []);

  const value = useMemo<ConnectionsContextValue>(
    () => ({
      statusOf: (id) => deriveConnectionStatus(monitors[id], terminals[id]),
      setTerminal,
      tests,
      setTest,
    }),
    [monitors, terminals, tests, setTerminal, setTest],
  );
  return <ConnectionsContext.Provider value={value}>{children}</ConnectionsContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useConnections(): ConnectionsContextValue {
  const ctx = useContext(ConnectionsContext);
  if (!ctx) throw new Error("useConnections must be used inside <ConnectionsProvider>");
  return ctx;
}

import type { Tone } from "../components/tones";
import type { ServerMonitor } from "../types/metrics";

/**
 * The one place that decides what "connected" means in the UI.
 *
 * VPSPilot keeps no idle connections. A server is connected only while something is actually
 * holding an authenticated SSH session to it: the monitor or an open terminal. A connection
 * test logs in and out again, so its result is deliberately NOT an input here; it is shown
 * separately as "Last test".
 */
export type ConnectionKind =
  | "not_connected"
  | "connecting"
  | "connected"
  | "monitoring"
  | "reconnecting"
  | "auth_failed"
  | "error";

/** What the Terminal page reports about its session for one server. */
export type TerminalSessionState =
  | { kind: "connecting" }
  | { kind: "connected" }
  | { kind: "closed" }
  | { kind: "error"; message: string };

export interface ConnectionStatus {
  kind: ConnectionKind;
  label: string;
  tone: Tone;
  /** A short explanation, e.g. the failure message. */
  detail?: string | undefined;
  /** True while the monitor is polling this server. */
  monitoring: boolean;
  /** True while an authenticated terminal session is open. */
  terminalOpen: boolean;
}

const VIEW: Record<ConnectionKind, { label: string; tone: Tone }> = {
  not_connected: { label: "Not connected", tone: "neutral" },
  connecting: { label: "Connecting…", tone: "warn" },
  connected: { label: "Connected", tone: "ok" },
  monitoring: { label: "Monitoring active", tone: "ok" },
  reconnecting: { label: "Reconnecting…", tone: "warn" },
  auth_failed: { label: "Authentication failed", tone: "bad" },
  error: { label: "Connection error", tone: "bad" },
};

/**
 * Combines the monitor and terminal state of one server. Precedence, strongest first:
 * live monitoring, an open terminal, reconnecting, connecting, failures, otherwise not connected.
 * A live session is never reported as a failure just because another channel failed.
 */
export function deriveConnectionStatus(
  monitor: ServerMonitor | undefined,
  terminal: TerminalSessionState | undefined,
): ConnectionStatus {
  const monitoring = monitor?.state === "online";
  const terminalOpen = terminal?.kind === "connected";
  const make = (kind: ConnectionKind, detail?: string): ConnectionStatus => ({
    kind,
    ...VIEW[kind],
    detail,
    monitoring,
    terminalOpen,
  });

  if (monitoring) {
    return make("monitoring", terminalOpen ? "An SSH terminal is also open." : undefined);
  }
  if (terminalOpen) return make("connected", "An SSH terminal session is open.");
  if (monitor?.state === "retrying") return make("reconnecting", monitor.message);
  if (monitor?.state === "connecting" || terminal?.kind === "connecting") {
    return make("connecting");
  }
  if (monitor?.state === "failed") {
    return make(monitor.cause === "auth_failed" ? "auth_failed" : "error", monitor.message);
  }
  if (terminal?.kind === "error") return make("error", terminal.message);
  return make("not_connected");
}

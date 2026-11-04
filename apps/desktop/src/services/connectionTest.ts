import type { Tone } from "../components/tones";
import type { ConnectionTestOutcome, ConnectionTestResult } from "../types/servers";

export interface OutcomeView {
  label: string;
  tone: Tone;
}

const VIEWS: Record<ConnectionTestOutcome, OutcomeView> = {
  success: { label: "Test passed", tone: "ok" },
  host_key_unknown: { label: "Host key not trusted", tone: "warn" },
  host_key_changed: { label: "Host key changed", tone: "bad" },
  auth_failed: { label: "Authentication failed", tone: "bad" },
  no_credential: { label: "No credential", tone: "warn" },
  unreachable: { label: "Unreachable", tone: "bad" },
  timeout: { label: "Timed out", tone: "bad" },
  error: { label: "Test failed", tone: "bad" },
};

export function outcomeView(outcome: ConnectionTestOutcome): OutcomeView {
  return VIEWS[outcome];
}

/** The user may trust a host key only right after the server presented an untrusted one. */
export function canTrustHostKey(result: ConnectionTestResult | undefined): boolean {
  return result?.outcome === "host_key_unknown" && result.hostKey !== undefined;
}

/** A changed key is never trusted in one step; the stored key must be forgotten first. */
export function isHostKeyMismatch(result: ConnectionTestResult | undefined): boolean {
  return result?.outcome === "host_key_changed";
}

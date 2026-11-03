import type { AlertSeverity, Container, LogLevel, Status } from "../types";

export type Tone = "ok" | "warn" | "bad" | "neutral";

export const statusTone: Record<Status, Tone> = { online: "ok", warning: "warn", offline: "bad" };
export const containerTone: Record<Container["state"], Tone> = {
  running: "ok",
  restarting: "warn",
  exited: "neutral",
};
export const logTone: Record<LogLevel, Tone> = {
  info: "neutral",
  debug: "neutral",
  warn: "warn",
  error: "bad",
};
export const severityTone: Record<AlertSeverity, Tone> = {
  critical: "bad",
  warning: "warn",
  info: "neutral",
};

import type { Status } from "../types";

export type Tone = "ok" | "warn" | "bad" | "neutral";

export const statusTone: Record<Status, Tone> = { online: "ok", warning: "warn", offline: "bad" };

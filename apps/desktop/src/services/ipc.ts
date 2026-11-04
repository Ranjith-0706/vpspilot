import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppError, AppErrorCode } from "../types/servers";

const CODES: AppErrorCode[] = [
  "validation",
  "not_found",
  "database",
  "credential",
  "connection",
  "internal",
];

/** Normalizes anything thrown by `invoke` into an AppError. Never includes input values. */
export function toAppError(err: unknown): AppError {
  if (typeof err === "object" && err !== null) {
    const e = err as Record<string, unknown>;
    if (typeof e.message === "string" && CODES.includes(e.code as AppErrorCode)) {
      return {
        code: e.code as AppErrorCode,
        message: e.message,
        ...(typeof e.field === "string" ? { field: e.field } : {}),
      };
    }
  }
  if (typeof err === "string") return { code: "internal", message: err };
  return { code: "internal", message: "An unexpected error occurred." };
}

/** Throws an AppError when not running inside the desktop app. */
export function requireDesktop(what: string): void {
  if (!isTauri()) {
    throw {
      code: "internal",
      message: `${what} is only available in the desktop app.`,
    } satisfies AppError;
  }
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  requireDesktop("This feature");
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toAppError(err);
  }
}

import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppError, AppErrorCode, ServerInput, ServerProfile } from "../types/servers";

const CODES: AppErrorCode[] = ["validation", "not_found", "database", "credential", "internal"];

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

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw {
      code: "internal",
      message: "Server management is only available in the desktop app.",
    } satisfies AppError;
  }
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toAppError(err);
  }
}

export const serversApi = {
  list: () => call<ServerProfile[]>("list_servers"),
  add: (input: ServerInput) => call<ServerProfile>("add_server", { input }),
  update: (id: string, input: ServerInput) => call<ServerProfile>("update_server", { id, input }),
  remove: (id: string) => call<void>("delete_server", { id }),
  /** Write-only. There is intentionally no way to read a credential back. */
  setCredential: (id: string, secret: string) =>
    call<ServerProfile>("set_server_credential", { id, secret }),
  clearCredential: (id: string) => call<ServerProfile>("clear_server_credential", { id }),
};

import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppInfo } from "../types";

/** Reads app metadata from the Rust layer; falls back when running in a plain browser. */
export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return { name: "VPSPilot", version: "dev (browser)" };
  try {
    return await invoke<AppInfo>("get_app_info");
  } catch (err) {
    throw new Error(
      `Failed to read app info: ${err instanceof Error ? err.message : String(err)}`,
      {
        cause: err,
      },
    );
  }
}

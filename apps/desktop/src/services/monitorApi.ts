import { Channel, invoke } from "@tauri-apps/api/core";
import { call, requireDesktop, toAppError } from "./ipc";
import type { MonitorEvent } from "../types/metrics";

/**
 * Starts (or restarts) monitoring a saved server. The backend picks the host, port and
 * credential itself and always runs the same fixed, read-only command; it refuses unless the
 * server's host key is already trusted.
 */
export async function startMonitoring(
  serverId: string,
  intervalSecs: number,
  onEvent: (event: MonitorEvent) => void,
): Promise<void> {
  requireDesktop("Monitoring");
  const channel = new Channel<MonitorEvent>();
  channel.onmessage = onEvent;
  try {
    await invoke<void>("start_monitoring", { id: serverId, intervalSecs, onEvent: channel });
  } catch (err) {
    throw toAppError(err);
  }
}

export const stopMonitoring = (serverId: string) => call<void>("stop_monitoring", { id: serverId });

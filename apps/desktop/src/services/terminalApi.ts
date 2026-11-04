import { Channel, invoke } from "@tauri-apps/api/core";
import { call, requireDesktop, toAppError } from "./ipc";

/** Events streamed from the Rust session. `data` is base64-encoded output. */
export type TerminalEvent = { type: "data"; data: string } | { type: "closed"; reason?: string };

/**
 * Opens a shell on a saved server. The backend picks the host, port and credential itself;
 * it refuses unless the server's host key is already trusted.
 */
export async function openTerminal(
  serverId: string,
  cols: number,
  rows: number,
  onEvent: (event: TerminalEvent) => void,
): Promise<string> {
  requireDesktop("The terminal");
  const channel = new Channel<TerminalEvent>();
  channel.onmessage = onEvent;
  try {
    return await invoke<string>("open_terminal", { id: serverId, cols, rows, onEvent: channel });
  } catch (err) {
    throw toAppError(err);
  }
}

export const writeTerminal = (sessionId: string, data: string) =>
  call<void>("terminal_write", { sessionId, data });

export const resizeTerminal = (sessionId: string, cols: number, rows: number) =>
  call<void>("terminal_resize", { sessionId, cols, rows });

export const closeTerminal = (sessionId: string) => call<void>("close_terminal", { sessionId });

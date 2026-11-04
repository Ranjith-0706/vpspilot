import type { HostKeyRecord, ServerProfile } from "../types/servers";

/**
 * Why a server cannot be connected to (terminal or monitoring) right now, or null if it can.
 * Mirrors the checks the backend enforces: a trusted host key and a stored credential.
 */
export function connectionBlocker(
  server: Pick<ServerProfile, "credentialRef">,
  hostKey: HostKeyRecord | undefined,
): string | null {
  if (!hostKey) {
    return "This server's host key is not trusted yet. Run a connection test on the Servers page and trust the host key first.";
  }
  if (!server.credentialRef) {
    return "No credential is stored for this server. Edit it on the Servers page to add one.";
  }
  return null;
}

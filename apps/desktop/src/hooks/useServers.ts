import { useCallback, useEffect, useState } from "react";
import { serversApi, toAppError } from "../services/serversApi";
import type { AppError, HostKeyRecord, ServerProfile } from "../types/servers";

export type ServersState =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; servers: ServerProfile[]; hostKeys: Record<string, HostKeyRecord> };

async function load(): Promise<Extract<ServersState, { status: "ready" }>> {
  const [servers, keys] = await Promise.all([serversApi.list(), serversApi.listHostKeys()]);
  return {
    status: "ready",
    servers,
    hostKeys: Object.fromEntries(keys.map((k) => [k.serverId, k])),
  };
}

export function useServers(): { state: ServersState; reload: () => Promise<void> } {
  const [state, setState] = useState<ServersState>({ status: "loading" });

  // Manual refresh (after a save/delete or a retry). Keeps showing current data while loading.
  const reload = useCallback(async () => {
    try {
      setState(await load());
    } catch (err) {
      setState({ status: "error", error: toAppError(err) });
    }
  }, []);

  // Initial load.
  useEffect(() => {
    let active = true;
    load()
      .then((ready) => active && setState(ready))
      .catch((err: unknown) => active && setState({ status: "error", error: toAppError(err) }));
    return () => {
      active = false;
    };
  }, []);

  return { state, reload };
}

import { useCallback, useEffect, useState } from "react";
import { serversApi, toAppError } from "../services/serversApi";
import type { AppError, ServerProfile } from "../types/servers";

export type ServersState =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; servers: ServerProfile[] };

export function useServers(): { state: ServersState; reload: () => Promise<void> } {
  const [state, setState] = useState<ServersState>({ status: "loading" });

  // Manual refresh (after a save/delete or a retry). Keeps showing current data while loading.
  const reload = useCallback(async () => {
    try {
      setState({ status: "ready", servers: await serversApi.list() });
    } catch (err) {
      setState({ status: "error", error: toAppError(err) });
    }
  }, []);

  // Initial load.
  useEffect(() => {
    let active = true;
    serversApi
      .list()
      .then((servers) => active && setState({ status: "ready", servers }))
      .catch((err: unknown) => active && setState({ status: "error", error: toAppError(err) }));
    return () => {
      active = false;
    };
  }, []);

  return { state, reload };
}

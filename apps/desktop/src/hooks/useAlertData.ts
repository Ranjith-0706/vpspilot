import { useCallback, useEffect, useState } from "react";
import { alertsApi } from "../services/alertsApi";
import { toAppError } from "../services/ipc";
import type { AlertEvent, AlertRule } from "../types/alerts";
import type { AppError } from "../types/servers";

export type AlertDataState =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; rules: AlertRule[]; events: AlertEvent[] };

async function load(): Promise<Extract<AlertDataState, { status: "ready" }>> {
  const [rules, events] = await Promise.all([alertsApi.listRules(), alertsApi.listEvents(200)]);
  return { status: "ready", rules, events };
}

/** Saved rules and alert history. Reloads on its own whenever `version` changes. */
export function useAlertData(version: number): {
  state: AlertDataState;
  reload: () => Promise<void>;
} {
  const [state, setState] = useState<AlertDataState>({ status: "loading" });

  const reload = useCallback(async () => {
    try {
      setState(await load());
    } catch (err) {
      setState({ status: "error", error: toAppError(err) });
    }
  }, []);

  useEffect(() => {
    let active = true;
    load()
      .then((ready) => active && setState(ready))
      .catch((err: unknown) => active && setState({ status: "error", error: toAppError(err) }));
    return () => {
      active = false;
    };
  }, [version]);

  return { state, reload };
}

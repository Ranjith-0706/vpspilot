import type { ActiveAlert, AlertEvent, AlertRule, AlertRuleInput } from "../types/alerts";
import { call } from "./ipc";

/** Rules are plain thresholds on fixed metrics. Nothing here runs anything on a server. */
export const alertsApi = {
  listRules: () => call<AlertRule[]>("list_alert_rules"),
  addRule: (input: AlertRuleInput) => call<AlertRule>("add_alert_rule", { input }),
  updateRule: (id: string, input: AlertRuleInput) =>
    call<AlertRule>("update_alert_rule", { id, input }),
  removeRule: (id: string) => call<void>("delete_alert_rule", { id }),
  /** Newest first. */
  listEvents: (limit?: number) => call<AlertEvent[]>("list_alert_events", { limit: limit ?? null }),
  clearEvents: () => call<void>("clear_alert_events"),
  listActive: () => call<ActiveAlert[]>("list_active_alerts"),
};

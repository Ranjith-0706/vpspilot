export type AlertMetric = "cpu" | "memory" | "disk";

/** "Alert when `metric` is at or above `threshold` percent for `consecutive` checks in a row." */
export interface AlertRule {
  id: string;
  /** `null` applies the rule to every server. */
  serverId: string | null;
  metric: AlertMetric;
  threshold: number;
  consecutive: number;
  enabled: boolean;
  /** Unix epoch milliseconds. */
  createdAt: number;
  updatedAt: number;
}

export interface AlertRuleInput {
  serverId: string | null;
  metric: AlertMetric;
  threshold: number;
  consecutive: number;
  enabled: boolean;
}

export type AlertEventState = "firing" | "resolved";

/**
 * One alert state change. `metric` and `threshold` are a snapshot from when it happened. A
 * resolved event with a `null` value means monitoring stopped or the rule was changed or removed.
 */
export interface AlertEvent {
  id: number;
  ruleId: string | null;
  serverId: string;
  metric: AlertMetric;
  threshold: number;
  state: AlertEventState;
  value: number | null;
  /** Unix epoch milliseconds. */
  at: number;
}

/** An alert that is firing right now. */
export interface ActiveAlert {
  ruleId: string;
  serverId: string;
  metric: AlertMetric;
  threshold: number;
  /** Unix epoch milliseconds. */
  since: number;
  value: number;
}

/** Raw rule form state. Numbers stay strings while the user types. */
export interface RuleFormValues {
  serverId: string;
  metric: AlertMetric;
  threshold: string;
  consecutive: string;
  enabled: boolean;
}

export type RuleFormField = keyof RuleFormValues;
export type RuleFormErrors = Partial<Record<RuleFormField, string>>;

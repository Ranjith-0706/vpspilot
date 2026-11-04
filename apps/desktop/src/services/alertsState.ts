import type { Tone } from "../components/tones";
import type {
  ActiveAlert,
  AlertEvent,
  AlertMetric,
  AlertRule,
  AlertRuleInput,
  RuleFormErrors,
  RuleFormValues,
} from "../types/alerts";
import { MONITOR_INTERVAL_SECS } from "./monitorState";

/** Mirrors the limits enforced by the Rust backend. */
export const LIMITS = {
  thresholdMin: 1,
  thresholdMax: 100,
  consecutiveMin: 1,
  consecutiveMax: 60,
} as const;

export const METRIC_LABEL: Record<AlertMetric, string> = {
  cpu: "CPU",
  memory: "Memory",
  disk: "Disk",
};

export const METRICS: AlertMetric[] = ["cpu", "memory", "disk"];

/** Starting points offered when there are no rules yet. Disk fills slowly, so one check is enough. */
export const RECOMMENDED_RULES: AlertRuleInput[] = [
  { serverId: null, metric: "cpu", threshold: 90, consecutive: 3, enabled: true },
  { serverId: null, metric: "memory", threshold: 90, consecutive: 3, enabled: true },
  { serverId: null, metric: "disk", threshold: 90, consecutive: 1, enabled: true },
];

const keyOf = (a: { ruleId: string | null; serverId: string }) => `${a.ruleId ?? ""}|${a.serverId}`;

/** Folds one alert event into the list of alerts that are firing right now. */
export function applyAlertEvent(active: ActiveAlert[], event: AlertEvent): ActiveAlert[] {
  if (event.ruleId === null) return active;
  const key = keyOf(event);
  const rest = active.filter((a) => keyOf(a) !== key);
  if (event.state === "resolved") return rest;
  return [
    ...rest,
    {
      ruleId: event.ruleId,
      serverId: event.serverId,
      metric: event.metric,
      threshold: event.threshold,
      since: event.at,
      value: event.value ?? 0,
    },
  ].sort((a, b) => a.since - b.since);
}

/** Monitoring of a server ended, so nothing is firing for it any more. */
export function dropServerAlerts(active: ActiveAlert[], serverId: string): ActiveAlert[] {
  const rest = active.filter((a) => a.serverId !== serverId);
  return rest.length === active.length ? active : rest;
}

/** "15 s", "2 min" or "1 min 30 s". */
export function formatWindow(checks: number, intervalSecs: number = MONITOR_INTERVAL_SECS): string {
  const total = Math.round(checks * intervalSecs);
  if (total < 60) return `${total} s`;
  const m = Math.floor(total / 60);
  const s = total % 60;
  return s === 0 ? `${m} min` : `${m} min ${s} s`;
}

export function describeCondition(
  rule: Pick<AlertRule, "metric" | "threshold" | "consecutive">,
): string {
  const base = `${METRIC_LABEL[rule.metric]} at or above ${rule.threshold}%`;
  return rule.consecutive > 1
    ? `${base} for ${rule.consecutive} checks in a row (about ${formatWindow(rule.consecutive)})`
    : base;
}

export function describeEvent(event: AlertEvent): string {
  const label = METRIC_LABEL[event.metric];
  if (event.state === "firing") {
    return `${label} reached ${Math.round(event.value ?? 0)}% (limit ${event.threshold}%)`;
  }
  return event.value === null
    ? `${label} alert ended (monitoring stopped or the rule changed)`
    : `${label} back down to ${Math.round(event.value)}%`;
}

export function severityOf(threshold: number): Tone {
  return threshold >= 90 ? "bad" : "warn";
}

export function initialRuleValues(rule?: AlertRule): RuleFormValues {
  return {
    serverId: rule?.serverId ?? "",
    metric: rule?.metric ?? "cpu",
    threshold: String(rule?.threshold ?? 90),
    consecutive: String(rule?.consecutive ?? 3),
    enabled: rule?.enabled ?? true,
  };
}

function wholeNumber(text: string): number | null {
  const t = text.trim();
  return /^\d{1,4}$/.test(t) ? Number(t) : null;
}

/** Validates the form; the backend checks the same limits again. */
export function validateRuleForm(values: RuleFormValues): {
  input?: AlertRuleInput;
  errors: RuleFormErrors;
} {
  const errors: RuleFormErrors = {};
  const threshold = wholeNumber(values.threshold);
  if (threshold === null || threshold < LIMITS.thresholdMin || threshold > LIMITS.thresholdMax) {
    errors.threshold = `Enter a whole number from ${LIMITS.thresholdMin} to ${LIMITS.thresholdMax}.`;
  }
  const consecutive = wholeNumber(values.consecutive);
  if (
    consecutive === null ||
    consecutive < LIMITS.consecutiveMin ||
    consecutive > LIMITS.consecutiveMax
  ) {
    errors.consecutive = `Enter a whole number from ${LIMITS.consecutiveMin} to ${LIMITS.consecutiveMax}.`;
  }
  if (threshold === null || consecutive === null || Object.keys(errors).length > 0) {
    return { errors };
  }
  return {
    errors,
    input: {
      serverId: values.serverId === "" ? null : values.serverId,
      metric: values.metric,
      threshold,
      consecutive,
      enabled: values.enabled,
    },
  };
}

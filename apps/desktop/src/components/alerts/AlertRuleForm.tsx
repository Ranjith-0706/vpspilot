import { useState, type FormEvent } from "react";
import { Button, Field, inputClass } from "../Field";
import { DIALOG_FORM, DialogBody, DialogFooter } from "../Dialog";
import {
  LIMITS,
  METRICS,
  METRIC_LABEL,
  describeCondition,
  initialRuleValues,
  validateRuleForm,
} from "../../services/alertsState";
import { toAppError } from "../../services/ipc";
import type {
  AlertMetric,
  AlertRule,
  AlertRuleInput,
  RuleFormErrors,
  RuleFormField,
  RuleFormValues,
} from "../../types/alerts";
import type { ServerProfile } from "../../types/servers";

const FIELDS: RuleFormField[] = ["serverId", "metric", "threshold", "consecutive", "enabled"];

export function AlertRuleForm({
  rule,
  servers,
  onSubmit,
  onCancel,
}: {
  rule?: AlertRule | undefined;
  servers: ServerProfile[];
  onSubmit: (input: AlertRuleInput) => Promise<void>;
  onCancel: () => void;
}) {
  const [values, setValues] = useState<RuleFormValues>(() => initialRuleValues(rule));
  const [errors, setErrors] = useState<RuleFormErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const set = <K extends RuleFormField>(key: K, value: RuleFormValues[K]) =>
    setValues((v) => ({ ...v, [key]: value }));

  const props = (key: RuleFormField) => ({
    id: `rule-${key}`,
    "aria-invalid": errors[key] ? true : undefined,
    "aria-describedby": errors[key] ? `rule-${key}-error` : undefined,
  });

  const preview = validateRuleForm(values).input;

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    const result = validateRuleForm(values);
    setErrors(result.errors);
    if (!result.input) return;

    setBusy(true);
    try {
      await onSubmit(result.input);
    } catch (err) {
      const e = toAppError(err);
      if (e.code === "validation" && e.field && (FIELDS as string[]).includes(e.field)) {
        setErrors({ [e.field]: e.message });
      } else {
        setFormError(e.message);
      }
      setBusy(false);
    }
  }

  return (
    <form onSubmit={(e) => void handleSubmit(e)} noValidate className={DIALOG_FORM}>
      <DialogBody>
        <Field id="rule-serverId" label="Applies to" error={errors.serverId}>
          <select
            {...props("serverId")}
            className={inputClass}
            value={values.serverId}
            onChange={(e) => set("serverId", e.target.value)}
          >
            <option value="">All servers</option>
            {servers.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </Field>

        <Field id="rule-metric" label="Watch" error={errors.metric}>
          <select
            {...props("metric")}
            className={inputClass}
            value={values.metric}
            onChange={(e) => set("metric", e.target.value as AlertMetric)}
          >
            {METRICS.map((m) => (
              <option key={m} value={m}>
                {METRIC_LABEL[m]}
                {m === "disk" ? " (the / filesystem)" : " usage"}
              </option>
            ))}
          </select>
        </Field>

        <div className="grid grid-cols-2 gap-3">
          <Field
            id="rule-threshold"
            label="Alert at or above (%)"
            error={errors.threshold}
            hint={`${LIMITS.thresholdMin} to ${LIMITS.thresholdMax}`}
          >
            <input
              {...props("threshold")}
              className={inputClass}
              value={values.threshold}
              inputMode="numeric"
              onChange={(e) => set("threshold", e.target.value)}
              autoFocus
            />
          </Field>
          <Field
            id="rule-consecutive"
            label="For this many checks"
            error={errors.consecutive}
            hint="Avoids alerts from brief spikes"
          >
            <input
              {...props("consecutive")}
              className={inputClass}
              value={values.consecutive}
              inputMode="numeric"
              onChange={(e) => set("consecutive", e.target.value)}
            />
          </Field>
        </div>

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={values.enabled}
            onChange={(e) => set("enabled", e.target.checked)}
          />
          Enabled
        </label>

        {preview && (
          <p className="rounded-md bg-surface-2 p-3 text-xs text-muted">
            Alert when {describeCondition(preview)}. It clears once the value is clearly back below
            the threshold. Alerts are only checked while the server is being monitored.
          </p>
        )}

        {formError && (
          <p role="alert" className="rounded-md bg-bad/10 p-3 text-sm text-bad">
            {formError}
          </p>
        )}
      </DialogBody>
      <DialogFooter>
        <Button onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? "Saving…" : rule ? "Save changes" : "Add rule"}
        </Button>
      </DialogFooter>
    </form>
  );
}

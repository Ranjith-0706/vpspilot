import { useEffect, useState } from "react";
import { Card, PageHeader } from "../components/ui";
import { Button, inputClass } from "../components/Field";
import { historyApi } from "../services/historyApi";
import {
  DEFAULT_RETENTION_DAYS,
  MAX_RETENTION_DAYS,
  MIN_RETENTION_DAYS,
  parseRetention,
} from "../services/historyState";
import { toAppError } from "../services/ipc";
import { useTheme } from "../hooks/useTheme";
import { useAppInfo } from "../hooks/useAppInfo";
import type { ThemePreference } from "../types";

const OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export default function SettingsPage() {
  const { preference, setPreference } = useTheme();
  const { info, error } = useAppInfo();
  const [days, setDays] = useState(String(DEFAULT_RETENTION_DAYS));
  const [saved, setSaved] = useState<number | null>(null);
  const [retentionMsg, setRetentionMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    let active = true;
    historyApi
      .getRetentionDays()
      .then((d) => {
        if (!active) return;
        setSaved(d);
        setDays(String(d));
      })
      .catch(() => undefined); // not in the desktop app, or the database is unavailable
    return () => {
      active = false;
    };
  }, []);

  const parsed = parseRetention(days);
  async function saveRetention() {
    if (parsed === null) return;
    try {
      const d = await historyApi.setRetentionDays(parsed);
      setSaved(d);
      setRetentionMsg({ ok: true, text: `Saved. Older history is being removed.` });
    } catch (err) {
      setRetentionMsg({ ok: false, text: toAppError(err).message });
    }
  }

  return (
    <>
      <PageHeader title="Settings" subtitle="Application preferences." />
      <div className="max-w-xl space-y-4">
        <Card title="Appearance">
          <div role="radiogroup" aria-label="Theme" className="flex gap-2">
            {OPTIONS.map((o) => (
              <button
                key={o.value}
                type="button"
                role="radio"
                aria-checked={preference === o.value}
                onClick={() => setPreference(o.value)}
                className={`rounded-md border px-4 py-2 text-sm font-medium ${
                  preference === o.value
                    ? "border-accent bg-accent/15 text-accent"
                    : "border-border text-muted hover:text-fg"
                }`}
              >
                {o.label}
              </button>
            ))}
          </div>
        </Card>
        <Card title="Monitoring history">
          <p className="mb-3 text-sm">
            While monitoring runs, VPSPilot stores one reading a minute per server on this computer
            so you can review charts after restarting. Older readings are removed automatically.
          </p>
          <label htmlFor="retention-days" className="mb-1 block text-sm font-medium">
            Keep history for (days)
          </label>
          <div className="flex items-center gap-2">
            <input
              id="retention-days"
              inputMode="numeric"
              className={`${inputClass} w-24`}
              value={days}
              onChange={(e) => {
                setDays(e.target.value);
                setRetentionMsg(null);
              }}
              aria-invalid={parsed === null}
            />
            <Button
              onClick={() => void saveRetention()}
              disabled={parsed === null || parsed === saved}
            >
              Save
            </Button>
          </div>
          <p className="mt-1 text-xs text-muted">
            {MIN_RETENTION_DAYS} to {MAX_RETENTION_DAYS} days. Shortening it deletes older history.
          </p>
          {parsed === null && (
            <p role="alert" className="mt-2 text-sm text-bad">
              Enter a whole number from {MIN_RETENTION_DAYS} to {MAX_RETENTION_DAYS}.
            </p>
          )}
          {retentionMsg && (
            <p
              role={retentionMsg.ok ? "status" : "alert"}
              className={`mt-2 text-sm ${retentionMsg.ok ? "text-ok" : "text-bad"}`}
            >
              {retentionMsg.text}
            </p>
          )}
        </Card>
        <Card title="About">
          <p className="text-sm">
            {error ? (
              <span className="text-bad">{error}</span>
            ) : info ? (
              `${info.name} v${info.version}`
            ) : (
              "Loading…"
            )}
          </p>
          <p className="mt-1 text-xs text-muted">Open-source VPS monitoring and management.</p>
        </Card>
      </div>
    </>
  );
}

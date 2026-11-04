import { Card, PageHeader } from "../components/ui";
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
                className={`rounded-lg border px-4 py-2 text-sm font-medium ${
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

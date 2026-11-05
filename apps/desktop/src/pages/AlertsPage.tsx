import { useState } from "react";
import { Badge, Card, PageHeader, Table } from "../components/ui";
import { Button } from "../components/Field";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Dialog } from "../components/Dialog";
import { AlertRuleForm } from "../components/alerts/AlertRuleForm";
import { useAlertData } from "../hooks/useAlertData";
import { useMonitoring } from "../hooks/useMonitoring";
import { useServers } from "../hooks/useServers";
import { alertsApi } from "../services/alertsApi";
import {
  METRIC_LABEL,
  RECOMMENDED_RULES,
  describeCondition,
  describeEvent,
  severityOf,
} from "../services/alertsState";
import { toAppError } from "../services/ipc";
import type { AlertRule, AlertRuleInput } from "../types/alerts";

type Modal =
  | { kind: "add" }
  | { kind: "edit"; id: string }
  | { kind: "delete"; id: string }
  | { kind: "clear" }
  | null;

const time = (ms: number) => new Date(ms).toLocaleString();

export default function AlertsPage() {
  const { activeAlerts, alertVersion } = useMonitoring();
  const { state: data, reload } = useAlertData(alertVersion);
  const { state: serversState } = useServers();
  const [modal, setModal] = useState<Modal>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const servers = serversState.status === "ready" ? serversState.servers : [];
  const serverName = (id: string | null) =>
    id === null ? "All servers" : (servers.find((s) => s.id === id)?.name ?? "Unknown server");

  const rules = data.status === "ready" ? data.rules : [];
  const modalRule =
    modal && (modal.kind === "edit" || modal.kind === "delete")
      ? rules.find((r) => r.id === modal.id)
      : undefined;

  async function saveRule(input: AlertRuleInput) {
    // Errors thrown here are shown inside the form.
    if (modal?.kind === "edit" && modalRule) await alertsApi.updateRule(modalRule.id, input);
    else await alertsApi.addRule(input);
    setModal(null);
    await reload();
  }

  async function toggle(rule: AlertRule) {
    setNotice(null);
    try {
      await alertsApi.updateRule(rule.id, {
        serverId: rule.serverId,
        metric: rule.metric,
        threshold: rule.threshold,
        consecutive: rule.consecutive,
        enabled: !rule.enabled,
      });
      await reload();
    } catch (err) {
      setNotice(toAppError(err).message);
    }
  }

  async function addRecommended() {
    setNotice(null);
    try {
      for (const rule of RECOMMENDED_RULES) await alertsApi.addRule(rule);
    } catch (err) {
      setNotice(toAppError(err).message);
    }
    await reload();
  }

  const addButton = (
    <Button
      variant="primary"
      onClick={() => setModal({ kind: "add" })}
      disabled={data.status !== "ready"}
    >
      Add rule
    </Button>
  );

  return (
    <>
      <PageHeader
        title="Alerts"
        subtitle="Get alerted when a server's CPU, memory or disk stays high. Rules are checked only while a server is being monitored on the Dashboard."
        actions={addButton}
      />

      {notice && (
        <p role="alert" className="mb-4 rounded-md bg-bad/10 p-3 text-sm text-bad">
          {notice}
        </p>
      )}

      {data.status === "loading" && (
        <p role="status" className="text-sm text-muted">
          Loading alerts…
        </p>
      )}

      {data.status === "error" && (
        <Card>
          <p role="alert" className="font-medium text-bad">
            Could not load alerts.
          </p>
          <p className="mt-1 text-sm text-muted">{data.error.message}</p>
          <Button className="mt-4" onClick={() => void reload()}>
            Retry
          </Button>
        </Card>
      )}

      {data.status === "ready" && (
        <div className="space-y-8">
          <section aria-labelledby="active-heading">
            <h2 id="active-heading" className="mb-3 text-lg font-semibold">
              Active now
            </h2>
            {activeAlerts.length === 0 ? (
              <Card>
                <p className="text-sm text-muted">Nothing is firing.</p>
              </Card>
            ) : (
              <div className="space-y-3">
                {activeAlerts.map((a) => (
                  <Card key={`${a.ruleId}|${a.serverId}`}>
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div>
                        <p className="font-medium">
                          {serverName(a.serverId)}: {METRIC_LABEL[a.metric]} is high
                        </p>
                        <p className="text-xs text-muted">
                          Reached {Math.round(a.value)}% (limit {a.threshold}%) · since{" "}
                          {time(a.since)}
                        </p>
                      </div>
                      <Badge tone={severityOf(a.threshold)}>firing</Badge>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </section>

          <section aria-labelledby="rules-heading">
            <h2 id="rules-heading" className="mb-3 text-lg font-semibold">
              Rules
            </h2>
            {rules.length === 0 ? (
              <Card className="py-8 text-center">
                <p className="text-lg font-medium">No alert rules yet</p>
                <p className="mx-auto mt-1 max-w-md text-sm text-muted">
                  Start with the recommended rules (CPU, memory and disk at 90% on all servers), or
                  add your own.
                </p>
                <div className="mt-5 flex justify-center gap-2">
                  <Button variant="primary" onClick={() => void addRecommended()}>
                    Add recommended rules
                  </Button>
                  <Button onClick={() => setModal({ kind: "add" })}>Add rule</Button>
                </div>
              </Card>
            ) : (
              <Table headers={["Applies to", "Condition", "Status", ""]}>
                {rules.map((r) => (
                  <tr key={r.id} className={r.enabled ? "" : "opacity-60"}>
                    <td className="px-3 py-2 font-medium">{serverName(r.serverId)}</td>
                    <td className="px-3 py-2">{describeCondition(r)}</td>
                    <td className="px-3 py-2">
                      <Badge tone={r.enabled ? "ok" : "neutral"}>
                        {r.enabled ? "Enabled" : "Disabled"}
                      </Badge>
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-right">
                      <RowAction
                        label={r.enabled ? "Disable" : "Enable"}
                        onClick={() => void toggle(r)}
                      />
                      <RowAction
                        label="Edit"
                        onClick={() => setModal({ kind: "edit", id: r.id })}
                      />
                      <RowAction
                        label="Delete"
                        danger
                        onClick={() => setModal({ kind: "delete", id: r.id })}
                      />
                    </td>
                  </tr>
                ))}
              </Table>
            )}
          </section>

          <section aria-labelledby="history-heading">
            <div className="mb-3 flex items-center justify-between gap-3">
              <h2 id="history-heading" className="text-lg font-semibold">
                History
              </h2>
              {data.events.length > 0 && (
                <Button onClick={() => setModal({ kind: "clear" })}>Clear history</Button>
              )}
            </div>
            {data.events.length === 0 ? (
              <Card>
                <p className="text-sm text-muted">
                  No alerts yet. When a rule fires or clears, it is recorded here.
                </p>
              </Card>
            ) : (
              <Card>
                <ul className="divide-y divide-border">
                  {data.events.map((e) => (
                    <li
                      key={e.id}
                      className="flex flex-wrap items-center justify-between gap-3 py-3"
                    >
                      <div>
                        <p className="text-sm font-medium">
                          {serverName(e.serverId)}: {describeEvent(e)}
                        </p>
                        <p className="text-xs text-muted">{time(e.at)}</p>
                      </div>
                      {e.state === "firing" ? (
                        <Badge tone={severityOf(e.threshold)}>fired</Badge>
                      ) : (
                        <Badge tone="ok">resolved</Badge>
                      )}
                    </li>
                  ))}
                </ul>
              </Card>
            )}
          </section>
        </div>
      )}

      {modal?.kind === "add" && (
        <Dialog title="Add alert rule" onClose={() => setModal(null)}>
          <AlertRuleForm servers={servers} onSubmit={saveRule} onCancel={() => setModal(null)} />
        </Dialog>
      )}
      {modal?.kind === "edit" && modalRule && (
        <Dialog title="Edit alert rule" onClose={() => setModal(null)}>
          <AlertRuleForm
            rule={modalRule}
            servers={servers}
            onSubmit={saveRule}
            onCancel={() => setModal(null)}
          />
        </Dialog>
      )}
      {modal?.kind === "delete" && modalRule && (
        <ActionDialog
          title="Delete this rule?"
          confirmLabel="Delete"
          action={async () => {
            await alertsApi.removeRule(modalRule.id);
            setModal(null);
            await reload();
          }}
          onCancel={() => setModal(null)}
        >
          <p>
            {serverName(modalRule.serverId)}: {describeCondition(modalRule)}. Past alerts stay in
            the history.
          </p>
        </ActionDialog>
      )}
      {modal?.kind === "clear" && (
        <ActionDialog
          title="Clear alert history?"
          confirmLabel="Clear"
          action={async () => {
            await alertsApi.clearEvents();
            setModal(null);
            await reload();
          }}
          onCancel={() => setModal(null)}
        >
          <p>This removes every recorded alert. Rules and anything firing now are not affected.</p>
        </ActionDialog>
      )}
    </>
  );
}

function RowAction({
  label,
  onClick,
  danger = false,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`ml-1 rounded px-2 py-1 text-xs font-medium hover:bg-surface-2 ${danger ? "text-bad" : "text-accent"}`}
    >
      {label}
    </button>
  );
}

function ActionDialog({
  title,
  confirmLabel,
  action,
  onCancel,
  children,
}: {
  title: string;
  confirmLabel: string;
  action: () => Promise<void>;
  onCancel: () => void;
  children: React.ReactNode;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(toAppError(err).message);
      setBusy(false);
    }
  }

  return (
    <ConfirmDialog
      title={title}
      confirmLabel={confirmLabel}
      busy={busy}
      error={error}
      onConfirm={() => void confirm()}
      onCancel={onCancel}
    >
      {children}
    </ConfirmDialog>
  );
}

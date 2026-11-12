import { useEffect, useState } from "react";
import { Card } from "../ui";
import { Button, inputClass } from "../Field";
import { HistoryChart } from "./HistoryChart";
import { historyApi } from "../../services/historyApi";
import { toAppError } from "../../services/ipc";
import {
  DEFAULT_RETENTION_DAYS,
  formatAge,
  historyNote,
  isCurrent,
  loadAxisMax,
  rangesFor,
} from "../../services/historyState";
import type { HistoryRange } from "../../types/history";
import type { ServerProfile } from "../../types/servers";

/** Reload while the tab is open, so history recorded by running monitors appears by itself. */
const REFRESH_MS = 60_000;

type Outcome = { status: "ready"; range: HistoryRange } | { status: "error"; message: string };

/**
 * Stored history of one server. Everything here is recorded data from the past; the status of
 * a server right now is shown on the Live tab.
 */
export function HistoryView({ servers }: { servers: ServerProfile[] }) {
  const [serverId, setServerId] = useState(servers[0]?.id ?? "");
  const [retention, setRetention] = useState(DEFAULT_RETENTION_DAYS);
  const ranges = rangesFor(retention);
  const [secs, setSecs] = useState(ranges[0]?.secs ?? 3600);
  // The newest reply, tagged with the selection it answers. A reply for an older selection is
  // never shown as the current one (it simply counts as "still loading").
  const [reply, setReply] = useState<{ key: string; outcome: Outcome } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [refreshes, setRefreshes] = useState(0);

  // A deleted or missing selection falls back to the first server.
  const selected = servers.find((s) => s.id === serverId) ?? servers[0];
  const selectedId = selected?.id;

  useEffect(() => {
    let active = true;
    historyApi
      .getRetentionDays()
      .then((d) => active && setRetention(d))
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  // Keep the chosen range valid if the retention setting shrinks.
  const rangeSecs = ranges.some((r) => r.secs === secs) ? secs : (ranges[0]?.secs ?? 3600);
  const key = `${selectedId}:${rangeSecs}`;

  useEffect(() => {
    if (!selectedId) return;
    let active = true;
    const run = () =>
      historyApi
        .get(selectedId, rangeSecs)
        .then((range) => {
          if (!active) return;
          setReply({ key, outcome: { status: "ready", range } });
          setNow(Date.now());
        })
        .catch((err: unknown) => {
          if (active)
            setReply({ key, outcome: { status: "error", message: toAppError(err).message } });
        });
    void run();
    const timer = setInterval(() => void run(), REFRESH_MS);
    return () => {
      active = false; // ignore any reply still in flight for a previous selection
      clearInterval(timer);
    };
  }, [selectedId, rangeSecs, key, refreshes]);

  const outcome = reply?.key === key ? reply.outcome : undefined;
  const loading = outcome === undefined;

  const selection = selectedId ? { serverId: selectedId, secs: rangeSecs } : undefined;
  const range = outcome?.status === "ready" ? outcome.range : undefined;
  const shown = isCurrent(range, selection) ? range : undefined;
  const note = shown ? historyNote(shown, shown.retentionDays) : undefined;

  return (
    <div className="space-y-4">
      <p
        role="note"
        className="rounded-md border border-border bg-surface-2 p-3 text-xs text-muted"
      >
        Historical data: readings recorded while monitoring was running. It is not the current
        status of a server. Use the Live tab to see how a server is doing now.
      </p>

      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label htmlFor="history-server" className="mb-1 block text-xs font-medium">
            Server
          </label>
          <select
            id="history-server"
            className={inputClass}
            value={selectedId ?? ""}
            onChange={(e) => setServerId(e.target.value)}
          >
            {servers.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="history-range" className="mb-1 block text-xs font-medium">
            Time range
          </label>
          <select
            id="history-range"
            className={inputClass}
            value={rangeSecs}
            onChange={(e) => setSecs(Number(e.target.value))}
          >
            {ranges.map((r) => (
              <option key={r.secs} value={r.secs}>
                {r.label}
              </option>
            ))}
          </select>
        </div>
        <Button onClick={() => setRefreshes((n) => n + 1)}>Refresh</Button>
      </div>

      {loading && (
        <p role="status" className="text-sm text-muted">
          Loading history…
        </p>
      )}
      {outcome?.status === "error" && (
        <Card>
          <p role="alert" className="text-sm text-bad">
            History could not be loaded: {outcome.message}
          </p>
        </Card>
      )}

      {shown && (
        <>
          <p className="text-xs text-muted">
            {shown.lastSampleAt === null
              ? "Nothing has been recorded for this server in this range."
              : `${shown.sampleCount} ${shown.sampleCount === 1 ? "sample" : "samples"} recorded; the latest was ${formatAge(shown.lastSampleAt, now)} (${new Date(shown.lastSampleAt).toLocaleString()}).`}
          </p>
          {note && note.kind !== "none" && (
            <p role="status" className="rounded-md bg-warn/10 p-3 text-sm text-warn">
              {note.text}
            </p>
          )}
          {shown.points.length > 0 && (
            <div className="grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2">
              <HistoryChart
                title="CPU history"
                {...chartBase(shown)}
                yMax={100}
                unit="%"
                series={[{ label: "CPU", color: "var(--accent)", pick: (p) => p.cpuPercent }]}
              />
              <HistoryChart
                title="Memory history"
                {...chartBase(shown)}
                yMax={100}
                unit="%"
                series={[{ label: "Memory", color: "var(--ok)", pick: (p) => p.memUsedPercent }]}
              />
              <HistoryChart
                title="Disk history (/)"
                {...chartBase(shown)}
                yMax={100}
                unit="%"
                series={[{ label: "Disk", color: "var(--warn)", pick: (p) => p.diskUsedPercent }]}
              />
              <HistoryChart
                title="Load average history"
                {...chartBase(shown)}
                yMax={loadAxisMax(shown.points)}
                unit="load"
                series={[
                  { label: "1 min", color: "var(--accent)", pick: (p) => p.load1 },
                  { label: "5 min", color: "var(--ok)", pick: (p) => p.load5 },
                  { label: "15 min", color: "var(--warn)", pick: (p) => p.load15 },
                ]}
              />
            </div>
          )}
        </>
      )}
    </div>
  );
}

function chartBase(r: HistoryRange) {
  return { points: r.points, bucketMs: r.bucketMs, from: r.from, to: r.to };
}

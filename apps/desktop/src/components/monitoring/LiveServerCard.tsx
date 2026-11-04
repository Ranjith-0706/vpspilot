import { Badge, Card, UsageBar } from "../ui";
import { Button } from "../Field";
import { Sparkline } from "./Sparkline";
import { formatKib, formatUptime } from "../../services/monitorState";
import type { Tone } from "../tones";
import type { MonitorStatus, ServerMonitor } from "../../types/metrics";
import type { ServerProfile } from "../../types/servers";

const STATUS: Record<MonitorStatus, { label: string; tone: Tone }> = {
  connecting: { label: "Connecting…", tone: "warn" },
  online: { label: "Online", tone: "ok" },
  retrying: { label: "Reconnecting…", tone: "warn" },
  failed: { label: "Failed", tone: "bad" },
  stopped: { label: "Stopped", tone: "neutral" },
};

export function LiveServerCard({
  server,
  monitor,
  blocker,
  onStart,
  onStop,
}: {
  server: ServerProfile;
  monitor: ServerMonitor | undefined;
  /** Why monitoring cannot be started right now, if it can't. */
  blocker: string | null;
  onStart: () => void;
  onStop: () => void;
}) {
  const active =
    monitor !== undefined &&
    (monitor.state === "connecting" || monitor.state === "online" || monitor.state === "retrying");
  const latest = monitor?.latest;
  const stale = latest !== undefined && monitor?.state !== "online";
  const status = monitor
    ? STATUS[monitor.state]
    : { label: "Not monitored", tone: "neutral" as Tone };

  return (
    <Card>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="truncate font-semibold">{server.name}</h3>
          <p className="truncate text-xs text-muted">
            {server.username}@{server.host}:{server.port}
          </p>
        </div>
        <Badge tone={status.tone}>{status.label}</Badge>
      </div>

      {monitor?.message && (monitor.state === "failed" || monitor.state === "retrying") && (
        <p
          role={monitor.state === "failed" ? "alert" : "status"}
          className={`mt-3 rounded-lg p-2 text-xs ${monitor.state === "failed" ? "bg-bad/10 text-bad" : "bg-warn/10 text-warn"}`}
        >
          {monitor.message}
        </p>
      )}
      {monitor?.state === "stopped" && monitor.message && (
        <p className="mt-3 text-xs text-muted">{monitor.message}</p>
      )}

      {latest && (
        <div className={`mt-4 space-y-3 ${stale ? "opacity-50" : ""}`}>
          <UsageBar label="CPU" percent={Math.round(latest.cpuPercent)} />
          <UsageBar label="Memory" percent={Math.round(latest.memUsedPercent)} />
          <UsageBar label="Disk (/)" percent={Math.round(latest.diskUsedPercent)} />
          <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <dt className="text-muted">Load (1/5/15m)</dt>
            <dd>
              {latest.load1} / {latest.load5} / {latest.load15}
            </dd>
            <dt className="text-muted">Cores</dt>
            <dd>{latest.cores}</dd>
            <dt className="text-muted">Memory</dt>
            <dd>
              {formatKib(latest.memTotalKib - latest.memAvailableKib)} of{" "}
              {formatKib(latest.memTotalKib)}
            </dd>
            <dt className="text-muted">Disk</dt>
            <dd>
              {formatKib(latest.diskUsedKib)} of {formatKib(latest.diskTotalKib)}
            </dd>
            {latest.swapTotalKib > 0 && (
              <>
                <dt className="text-muted">Swap</dt>
                <dd>
                  {formatKib(latest.swapUsedKib)} of {formatKib(latest.swapTotalKib)}
                </dd>
              </>
            )}
            <dt className="text-muted">Uptime</dt>
            <dd>{formatUptime(latest.uptimeSecs)}</dd>
          </dl>
          <div className="flex items-center gap-4 text-xs text-muted">
            <div>
              <p>CPU trend</p>
              <Sparkline values={monitor?.history.map((h) => h.cpu) ?? []} />
            </div>
            <div>
              <p>Memory trend</p>
              <Sparkline values={monitor?.history.map((h) => h.mem) ?? []} />
            </div>
          </div>
          <p className="text-xs text-muted">
            {stale ? "Last seen" : "Updated"} {new Date(latest.sampledAt).toLocaleTimeString()}
          </p>
        </div>
      )}

      {blocker && !active && (
        <p role="status" className="mt-3 rounded-lg bg-warn/10 p-2 text-xs text-warn">
          {blocker}
        </p>
      )}

      <div className="mt-4">
        {active ? (
          <Button variant="secondary" onClick={onStop}>
            Stop monitoring
          </Button>
        ) : (
          <Button variant="primary" onClick={onStart} disabled={blocker !== null}>
            {monitor ? "Start again" : "Start monitoring"}
          </Button>
        )}
      </div>
    </Card>
  );
}

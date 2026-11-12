import { useState } from "react";
import { Link } from "react-router-dom";
import { Card, MockBadge, PageHeader, StatCard } from "../components/ui";
import { Button } from "../components/Field";
import { LiveServerCard } from "../components/monitoring/LiveServerCard";
import { DemoDashboard } from "../demo/DemoDashboard";
import { useMonitoring } from "../hooks/useMonitoring";
import { useServers } from "../hooks/useServers";
import { connectionBlocker } from "../services/serverReadiness";
import { summarizeLive } from "../services/monitorState";
import { ROUTES } from "../routes";

const HIDE_DEMO_KEY = "vpspilot.hideDemo";

function readHideDemo(): boolean {
  try {
    return localStorage.getItem(HIDE_DEMO_KEY) === "1";
  } catch {
    return false;
  }
}

export default function DashboardPage() {
  const { state } = useServers();
  const { monitors, start, stop, activeAlerts } = useMonitoring();
  const [hideDemo, setHideDemo] = useState(readHideDemo);

  const servers = state.status === "ready" ? state.servers : [];
  const hostKeys = state.status === "ready" ? state.hostKeys : {};
  const live = summarizeLive(servers.map((s) => monitors[s.id]));
  const shownAlerts = activeAlerts.filter((a) => servers.some((s) => s.id === a.serverId));

  function toggleDemo() {
    const next = !hideDemo;
    setHideDemo(next);
    try {
      localStorage.setItem(HIDE_DEMO_KEY, next ? "1" : "0");
    } catch {
      /* storage unavailable: the choice just won't persist */
    }
  }

  return (
    <>
      <PageHeader
        title="Dashboard"
        subtitle="Live status of your servers. Monitoring is read-only and runs only while the app is open."
      />

      {state.status === "loading" && <p className="text-sm text-muted">Loading servers…</p>}
      {state.status === "error" && (
        <Card>
          <p role="alert" className="text-sm text-bad">
            {state.error.message}
          </p>
        </Card>
      )}
      {state.status === "ready" && servers.length === 0 && (
        <Card className="max-w-xl">
          <p className="text-sm">
            You have no servers yet.{" "}
            <Link to={ROUTES.servers.path} className="text-accent underline">
              Add one on the Servers page
            </Link>{" "}
            to see live CPU, memory and disk here.
          </p>
        </Card>
      )}

      {state.status === "ready" && servers.length > 0 && (
        <>
          <div className="grid grid-cols-2 gap-3 min-[900px]:grid-cols-4">
            <StatCard
              label="Servers online"
              value={`${live.online}/${live.monitored}`}
              hint={
                live.monitored === 0
                  ? "Start monitoring a server below"
                  : `${live.monitored} of ${servers.length} monitored`
              }
            />
            <StatCard
              label="CPU"
              value={live.avgCpu === null ? "—" : `${live.avgCpu}%`}
              {...(live.avgCpu !== null ? { percent: live.avgCpu } : {})}
              hint="Average of online servers"
            />
            <StatCard
              label="Memory"
              value={live.avgMem === null ? "—" : `${live.avgMem}%`}
              {...(live.avgMem !== null ? { percent: live.avgMem } : {})}
              hint="Average of online servers"
            />
            <StatCard
              label="Disk"
              value={live.avgDisk === null ? "—" : `${live.avgDisk}%`}
              {...(live.avgDisk !== null ? { percent: live.avgDisk } : {})}
              hint="Average of online servers"
            />
          </div>

          {shownAlerts.length > 0 && (
            <p role="status" className="mt-4 rounded-md bg-bad/10 p-3 text-sm text-bad">
              {shownAlerts.length} {shownAlerts.length === 1 ? "alert is" : "alerts are"} firing.{" "}
              <Link to={ROUTES.alerts.path} className="font-medium underline">
                View alerts
              </Link>
            </p>
          )}

          <div className="mt-4 grid grid-cols-1 gap-3 min-[1100px]:grid-cols-2 min-[1700px]:grid-cols-3">
            {servers.map((s) => (
              <LiveServerCard
                key={s.id}
                server={s}
                monitor={monitors[s.id]}
                blocker={connectionBlocker(s, hostKeys[s.id])}
                alerts={activeAlerts.filter((a) => a.serverId === s.id)}
                onStart={() => void start(s.id)}
                onStop={() => void stop(s.id)}
              />
            ))}
          </div>
        </>
      )}

      <section className="mt-10 border-t border-border pt-6" aria-label="Demo data">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold">Demo data</h2>
            <MockBadge />
          </div>
          <Button onClick={toggleDemo}>{hideDemo ? "Show demo data" : "Hide demo data"}</Button>
        </div>
        {!hideDemo && (
          <>
            <p className="mb-4 rounded-md bg-warn/10 p-3 text-sm text-warn">
              Everything in this section is sample data. It is not read from your servers.
            </p>
            <DemoDashboard />
          </>
        )}
      </section>
    </>
  );
}

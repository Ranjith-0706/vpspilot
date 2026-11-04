import { Badge, Card, StatCard, UsageBar } from "../components/ui";
import { statusTone } from "../components/tones";
import { mockServers } from "./demoData";
import { summarizeFleet } from "./fleet";

const fleet = summarizeFleet(mockServers);

/** The sample dashboard. Everything here is demo data, never read from saved servers. */
export function DemoDashboard() {
  return (
    <>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <StatCard
          label="CPU"
          value={`${fleet.avgCpu}%`}
          percent={fleet.avgCpu}
          hint="Average of servers that are up"
        />
        <StatCard
          label="RAM"
          value={`${fleet.avgRam}%`}
          percent={fleet.avgRam}
          hint="Average of servers that are up"
        />
        <StatCard
          label="Disk"
          value={`${fleet.avgDisk}%`}
          percent={fleet.avgDisk}
          hint="Average of servers that are up"
        />
        <StatCard
          label="Servers up"
          value={`${fleet.reachable}/${fleet.total}`}
          hint={`${fleet.online} online · ${fleet.warning} warning · ${fleet.offline} offline`}
        />
      </div>
      <Card title="Demo servers" className="mt-6">
        <ul className="divide-y divide-border">
          {mockServers.map((s) => (
            <li
              key={s.id}
              className="grid grid-cols-[1fr_auto] items-center gap-4 py-3 md:grid-cols-[1fr_2fr_auto]"
            >
              <div>
                <p className="font-medium">{s.name}</p>
                <p className="text-xs text-muted">{s.region}</p>
              </div>
              <div className="hidden md:block">
                <UsageBar label="CPU" percent={s.cpuPercent} />
              </div>
              <Badge tone={statusTone[s.status]}>{s.status}</Badge>
            </li>
          ))}
        </ul>
      </Card>
    </>
  );
}

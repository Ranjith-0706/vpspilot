import { Badge, Card, PageHeader } from "../components/ui";
import { severityTone } from "../components/tones";
import { mockAlerts } from "../demo/demoData";

export default function AlertsPage() {
  return (
    <>
      <PageHeader title="Alerts" subtitle="Recent alerts." mock />
      <div className="space-y-3">
        {mockAlerts.map((a) => (
          <Card key={a.id} className={a.resolved ? "opacity-60" : ""}>
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="font-medium">{a.title}</p>
                <p className="text-xs text-muted">
                  {a.server} · {a.time}
                </p>
              </div>
              <div className="flex gap-2">
                <Badge tone={severityTone[a.severity]}>{a.severity}</Badge>
                {a.resolved && <Badge tone="ok">resolved</Badge>}
              </div>
            </div>
          </Card>
        ))}
      </div>
    </>
  );
}

import { Badge, Card } from "../ui";
import { Button } from "../Field";
import type { ConnectionState, ServerProfile } from "../../types/servers";

// Real connectivity is not implemented yet, so every profile is not connected.
const CONNECTION: ConnectionState = "not_connected";

export function ConnectionBadge({ state = CONNECTION }: { state?: ConnectionState }) {
  return <Badge tone="neutral">{state === "not_connected" ? "Not connected" : state}</Badge>;
}

const fmt = (ms: number) => new Date(ms).toLocaleString();

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <dt className="text-xs text-muted">{label}</dt>
      <dd className="mt-0.5 break-words text-sm">{children}</dd>
    </div>
  );
}

export function ServerDetails({
  server,
  extra,
  onClose,
  onEdit,
  onDelete,
}: {
  server: ServerProfile;
  extra?: React.ReactNode;
  onClose: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <Card>
      <div className="mb-4 flex items-start justify-between gap-2">
        <div>
          <h2 className="text-lg font-semibold">{server.name}</h2>
          <div className="mt-1">
            <ConnectionBadge />
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close details"
          className="text-muted hover:text-fg"
        >
          ✕
        </button>
      </div>
      <dl className="space-y-3">
        <Row label="Host">{server.host}</Row>
        <Row label="SSH port">{server.port}</Row>
        <Row label="SSH username">{server.username}</Row>
        <Row label="Authentication">{server.authMethod === "ssh_key" ? "SSH key" : "Password"}</Row>
        <Row label="Credential">
          {server.credentialRef ? (
            <>
              <Badge tone="ok">Stored in OS credential store</Badge>
              <p className="mt-1 font-mono text-xs text-muted">{server.credentialRef}</p>
            </>
          ) : (
            <Badge tone="warn">No credential stored</Badge>
          )}
        </Row>
        <Row label="Description">
          {server.description ?? <span className="text-muted">None</span>}
        </Row>
        <Row label="Created">{fmt(server.createdAt)}</Row>
        <Row label="Updated">{fmt(server.updatedAt)}</Row>
        <Row label="ID">
          <span className="font-mono text-xs">{server.id}</span>
        </Row>
      </dl>
      {extra}
      <div className="mt-5 flex gap-2">
        <Button onClick={onEdit}>Edit</Button>
        <Button variant="danger" onClick={onDelete}>
          Delete
        </Button>
      </div>
    </Card>
  );
}

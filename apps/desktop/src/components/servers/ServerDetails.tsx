import { Badge, Card } from "../ui";
import { Button } from "../Field";
import type { ConnectionState, ServerProfile } from "../../types/servers";

// Real connectivity is not implemented yet, so every profile is not connected.
const CONNECTION: ConnectionState = "not_connected";

export function ConnectionBadge({ state = CONNECTION }: { state?: ConnectionState }) {
  return <Badge tone="neutral">{state === "not_connected" ? "Not connected" : state}</Badge>;
}

const fmt = (ms: number) => new Date(ms).toLocaleString();

function Row({
  label,
  children,
  wide = false,
}: {
  label: string;
  children: React.ReactNode;
  wide?: boolean;
}) {
  return (
    <div className={wide ? "col-span-2 sm:col-span-3 xl:col-span-2" : ""}>
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
      <div className="mb-3 flex items-start justify-between gap-2">
        <div>
          <h2 className="text-base font-semibold">{server.name}</h2>
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
      {extra}
      <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-2.5 border-t border-border pt-4 sm:grid-cols-3 xl:grid-cols-2">
        <Row label="Host">{server.host}</Row>
        <Row label="SSH port">{server.port}</Row>
        <Row label="SSH username">{server.username}</Row>
        <Row label="Authentication">{server.authMethod === "ssh_key" ? "SSH key" : "Password"}</Row>
        <Row label="Credential" wide>
          {server.credentialRef ? (
            <>
              <Badge tone="ok">Stored in OS credential store</Badge>
              <p className="mt-1 font-mono text-xs text-muted">{server.credentialRef}</p>
            </>
          ) : (
            <Badge tone="warn">No credential stored</Badge>
          )}
        </Row>
        <Row label="Description" wide>
          {server.description ?? <span className="text-muted">None</span>}
        </Row>
        <Row label="Created">{fmt(server.createdAt)}</Row>
        <Row label="Updated">{fmt(server.updatedAt)}</Row>
        <Row label="ID" wide>
          <span className="font-mono text-xs">{server.id}</span>
        </Row>
      </dl>
      <div className="mt-4 flex gap-2 border-t border-border pt-4">
        <Button onClick={onEdit}>Edit</Button>
        <Button variant="danger" onClick={onDelete}>
          Delete
        </Button>
      </div>
    </Card>
  );
}

import { useState } from "react";
import { Card, PageHeader, Table } from "../components/ui";
import { Badge } from "../components/ui";
import { Button } from "../components/Field";
import { Dialog, DialogBody, DialogFooter } from "../components/Dialog";
import { ServerForm, type ServerFormSubmit } from "../components/servers/ServerForm";
import { ConnectionBadge, ServerDetails } from "../components/servers/ServerDetails";
import { ConnectionTestPanel } from "../components/servers/ConnectionTestPanel";
import { outcomeView } from "../services/connectionTest";
import { useConnections } from "../hooks/useConnections";
import { useServers } from "../hooks/useServers";
import { serversApi, toAppError } from "../services/serversApi";
import type { ServerProfile } from "../types/servers";

type Modal = { kind: "add" } | { kind: "edit"; id: string } | { kind: "delete"; id: string } | null;

export default function ServersPage() {
  const { state, reload } = useServers();
  const [modal, setModal] = useState<Modal>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // Connection status and test results are shared with the other pages (in memory only).
  const { statusOf, tests, setTest } = useConnections();

  const servers = state.status === "ready" ? state.servers : [];
  const hostKeys = state.status === "ready" ? state.hostKeys : {};
  const find = (id: string | undefined) => servers.find((s) => s.id === id);
  const selected = find(selectedId ?? undefined);
  const modalServer = modal && modal.kind !== "add" ? find(modal.id) : undefined;

  async function save({ input, secret, removeCredential }: ServerFormSubmit) {
    const editing = modal?.kind === "edit" ? modalServer : undefined;
    // Errors thrown here (validation, database) are shown inside the form.
    const saved = editing
      ? await serversApi.update(editing.id, input)
      : await serversApi.add(input);
    try {
      if (secret !== undefined) await serversApi.setCredential(saved.id, secret);
      else if (removeCredential && saved.credentialRef) await serversApi.clearCredential(saved.id);
    } catch (err) {
      setNotice(
        `"${saved.name}" was saved, but its credential could not be updated: ${toAppError(err).message}`,
      );
    }
    setTest(saved.id, undefined); // an edit may have changed what a previous result referred to
    setModal(null);
    setSelectedId(saved.id);
    await reload();
  }

  const addButton = (
    <Button
      variant="primary"
      onClick={() => setModal({ kind: "add" })}
      disabled={state.status === "error"}
    >
      Add Server
    </Button>
  );

  return (
    <>
      <PageHeader
        title="Servers"
        subtitle="Manage your VPS servers, monitor system resources, access SSH terminals, and inspect Docker containers."
        actions={addButton}
      />

      {notice && (
        <div
          role="status"
          className="mb-4 flex items-start justify-between gap-3 rounded-md bg-warn/10 p-3 text-sm text-warn"
        >
          <span>{notice}</span>
          <button
            type="button"
            onClick={() => setNotice(null)}
            aria-label="Dismiss"
            className="shrink-0"
          >
            ✕
          </button>
        </div>
      )}

      {state.status === "loading" && (
        <p role="status" className="text-sm text-muted">
          Loading servers…
        </p>
      )}

      {state.status === "error" && (
        <Card>
          <p role="alert" className="font-medium text-bad">
            {state.error.code === "database"
              ? "The server database is unavailable."
              : "Could not load servers."}
          </p>
          <p className="mt-1 text-sm text-muted">{state.error.message}</p>
          <Button className="mt-4" onClick={() => void reload()}>
            Retry
          </Button>
        </Card>
      )}

      {state.status === "ready" && servers.length === 0 && (
        <Card className="py-10 text-center">
          <p className="text-lg font-medium">No servers configured</p>
          <p className="mx-auto mt-1 max-w-md text-sm text-muted">
            Add a server profile to get started. Profiles are stored locally on this computer;
            passwords and keys go to the OS credential store, never to the database.
          </p>
          <div className="mt-5">{addButton}</div>
        </Card>
      )}

      {state.status === "ready" && servers.length > 0 && (
        <div className={`grid gap-4 ${selected ? "xl:grid-cols-[1fr_22rem]" : ""}`}>
          <Table
            headers={[
              "Name",
              "Host",
              "User",
              "Auth",
              { label: "Connection", className: "hidden lg:table-cell" },
              { label: "Last test", className: "hidden lg:table-cell" },
              "Credential",
              "",
            ]}
          >
            {servers.map((s) => (
              <tr key={s.id} className={s.id === selectedId ? "bg-accent/5" : ""}>
                <td className="whitespace-nowrap px-3 py-2 font-medium">{s.name}</td>
                <td className="whitespace-nowrap px-3 py-2 text-muted">
                  {s.host}:{s.port}
                </td>
                <td className="whitespace-nowrap px-3 py-2">{s.username}</td>
                <td className="whitespace-nowrap px-3 py-2">
                  {s.authMethod === "ssh_key" ? "SSH key" : "Password"}
                </td>
                <td className="hidden px-3 py-2 lg:table-cell">
                  <ConnectionBadge status={statusOf(s.id)} />
                </td>
                <td className="hidden whitespace-nowrap px-3 py-2 lg:table-cell">
                  {tests[s.id] ? (
                    <span title="A connection test logs in once and disconnects; it does not keep a connection open.">
                      <Badge tone={outcomeView(tests[s.id]!.outcome).tone}>
                        {outcomeView(tests[s.id]!.outcome).label}
                      </Badge>
                    </span>
                  ) : (
                    <span className="text-xs text-muted">Not tested</span>
                  )}
                </td>
                <td className="whitespace-nowrap px-3 py-2">
                  {s.credentialRef ? (
                    <Badge tone="ok">Stored</Badge>
                  ) : (
                    <Badge tone="warn">None</Badge>
                  )}
                </td>
                <td className="whitespace-nowrap px-3 py-2 text-right">
                  <RowAction label="Details" server={s} onClick={() => setSelectedId(s.id)} />
                  <RowAction
                    label="Edit"
                    server={s}
                    onClick={() => setModal({ kind: "edit", id: s.id })}
                  />
                  <RowAction
                    label="Delete"
                    server={s}
                    onClick={() => setModal({ kind: "delete", id: s.id })}
                    danger
                  />
                </td>
              </tr>
            ))}
          </Table>
          {selected && (
            <ServerDetails
              status={statusOf(selected.id)}
              extra={
                <ConnectionTestPanel
                  key={selected.id}
                  server={selected}
                  hostKey={hostKeys[selected.id]}
                  result={tests[selected.id]}
                  onResult={(r) => setTest(selected.id, r)}
                  onChanged={reload}
                />
              }
              server={selected}
              onClose={() => setSelectedId(null)}
              onEdit={() => setModal({ kind: "edit", id: selected.id })}
              onDelete={() => setModal({ kind: "delete", id: selected.id })}
            />
          )}
        </div>
      )}

      {modal?.kind === "add" && (
        <Dialog title="Add Server" onClose={() => setModal(null)}>
          <ServerForm onSubmit={save} onCancel={() => setModal(null)} />
        </Dialog>
      )}
      {modal?.kind === "edit" && modalServer && (
        <Dialog title={`Edit ${modalServer.name}`} onClose={() => setModal(null)}>
          <ServerForm server={modalServer} onSubmit={save} onCancel={() => setModal(null)} />
        </Dialog>
      )}
      {modal?.kind === "delete" && modalServer && (
        <DeleteDialog
          server={modalServer}
          onCancel={() => setModal(null)}
          onDeleted={async () => {
            setModal(null);
            if (selectedId === modalServer.id) setSelectedId(null);
            setTest(modalServer.id, undefined);
            await reload();
          }}
        />
      )}
    </>
  );
}

function RowAction({
  label,
  server,
  onClick,
  danger = false,
}: {
  label: string;
  server: ServerProfile;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={`${label} ${server.name}`}
      className={`ml-1 rounded px-2 py-1 text-xs font-medium hover:bg-surface-2 ${danger ? "text-bad" : "text-accent"}`}
    >
      {label}
    </button>
  );
}

function DeleteDialog({
  server,
  onCancel,
  onDeleted,
}: {
  server: ServerProfile;
  onCancel: () => void;
  onDeleted: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      await serversApi.remove(server.id);
      await onDeleted();
    } catch (err) {
      setError(toAppError(err).message);
      setBusy(false);
    }
  }

  return (
    <Dialog title={`Delete ${server.name}?`} onClose={onCancel}>
      <DialogBody>
        <p className="text-sm">
          This removes the saved profile{server.credentialRef ? " and its stored credential" : ""}{" "}
          from this computer. The server itself is not touched.
        </p>
        {error && (
          <p role="alert" className="rounded-md bg-bad/10 p-3 text-sm text-bad">
            {error}
          </p>
        )}
      </DialogBody>
      <DialogFooter>
        <Button onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button variant="danger" onClick={() => void confirm()} disabled={busy}>
          {busy ? "Deleting…" : "Delete"}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

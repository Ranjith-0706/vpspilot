import { useState } from "react";
import { Link } from "react-router-dom";
import { Card, PageHeader } from "../components/ui";
import { Button, inputClass } from "../components/Field";
import { TerminalView } from "../components/terminal/TerminalView";
import { useServers } from "../hooks/useServers";
import { connectionBlocker } from "../services/serverReadiness";
import { ROUTES } from "../routes";

export default function TerminalPage() {
  const { state } = useServers();
  const [selectedId, setSelectedId] = useState("");
  // `attempt` changes on every connect so the view remounts and opens a fresh session.
  const [session, setSession] = useState<{ serverId: string; attempt: number } | null>(null);

  const servers = state.status === "ready" ? state.servers : [];
  const hostKeys = state.status === "ready" ? state.hostKeys : {};
  const selected = servers.find((s) => s.id === selectedId);
  const active = session ? servers.find((s) => s.id === session.serverId) : undefined;

  const blocker = selected ? connectionBlocker(selected, hostKeys[selected.id]) : null;

  return (
    <>
      <PageHeader title="Terminal" subtitle="Open a shell on one of your saved servers over SSH." />

      {session && active ? (
        <TerminalView
          key={`${session.serverId}:${session.attempt}`}
          serverId={session.serverId}
          title={`${active.username}@${active.host}`}
          onDisconnect={() => setSession(null)}
          onReconnect={() =>
            setSession({ serverId: session.serverId, attempt: session.attempt + 1 })
          }
        />
      ) : (
        <Card className="max-w-xl">
          {state.status === "loading" && <p className="text-sm text-muted">Loading servers…</p>}
          {state.status === "error" && (
            <p role="alert" className="text-sm text-bad">
              {state.error.message}
            </p>
          )}
          {state.status === "ready" && servers.length === 0 && (
            <p className="text-sm">
              No servers yet.{" "}
              <Link to={ROUTES.servers.path} className="text-accent underline">
                Add one on the Servers page
              </Link>
              .
            </p>
          )}
          {state.status === "ready" && servers.length > 0 && (
            <div className="space-y-4">
              <div>
                <label htmlFor="terminal-server" className="mb-1 block text-sm font-medium">
                  Server
                </label>
                <select
                  id="terminal-server"
                  className={inputClass}
                  value={selectedId}
                  onChange={(e) => setSelectedId(e.target.value)}
                >
                  <option value="">Choose a server…</option>
                  {servers.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.username}@{s.host})
                    </option>
                  ))}
                </select>
              </div>
              {blocker && (
                <p role="status" className="rounded-md bg-warn/10 p-3 text-sm text-warn">
                  {blocker}{" "}
                  <Link to={ROUTES.servers.path} className="underline">
                    Go to Servers
                  </Link>
                </p>
              )}
              <Button
                variant="primary"
                disabled={!selected || blocker !== null}
                onClick={() => selected && setSession({ serverId: selected.id, attempt: 0 })}
              >
                Connect
              </Button>
              <p className="text-xs text-muted">
                This opens an interactive shell on the remote server. Anything you type runs there,
                with that account's permissions. Passphrase-protected private keys are not supported
                yet.
              </p>
            </div>
          )}
        </Card>
      )}
    </>
  );
}

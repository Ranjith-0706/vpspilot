import { useState } from "react";
import { Link } from "react-router-dom";
import { Badge, Card, PageHeader, Table } from "../components/ui";
import { Button, inputClass } from "../components/Field";
import { useServers } from "../hooks/useServers";
import { dockerApi } from "../services/dockerApi";
import {
  HEALTH_TONE,
  STATE_TONE,
  filterContainers,
  sortContainers,
  summarizeContainers,
} from "../services/dockerState";
import { toAppError } from "../services/ipc";
import { connectionBlocker } from "../services/serverReadiness";
import { ROUTES } from "../routes";
import type { DockerResult } from "../types/docker";

/** What the page knows about one server. Kept in memory for this session only. */
interface Load {
  busy: boolean;
  result?: DockerResult;
  /** A connection or login problem, as opposed to a Docker one (which is in `result`). */
  error?: string;
}

export default function DockerPage() {
  const { state } = useServers();
  const [chosen, setChosen] = useState<string>("");
  const [loads, setLoads] = useState<Record<string, Load>>({});
  const [query, setQuery] = useState("");

  const servers = state.status === "ready" ? state.servers : [];
  const hostKeys = state.status === "ready" ? state.hostKeys : {};
  const server = servers.find((s) => s.id === chosen) ?? servers[0];
  const load = server ? loads[server.id] : undefined;
  const blocker = server ? connectionBlocker(server, hostKeys[server.id]) : null;

  async function fetchContainers(id: string) {
    setLoads((prev) => ({ ...prev, [id]: { ...prev[id], busy: true } }));
    try {
      const result = await dockerApi.listContainers(id);
      setLoads((prev) => ({ ...prev, [id]: { busy: false, result } }));
    } catch (err) {
      // Keep the last good list on screen, marked by the error above it.
      setLoads((prev) => ({
        ...prev,
        [id]: { ...prev[id], busy: false, error: toAppError(err).message },
      }));
    }
  }

  const result = load?.result;
  const containers = result?.outcome === "ok" ? sortContainers(result.containers) : [];
  const summary = summarizeContainers(containers);
  const shown = filterContainers(containers, query);

  return (
    <>
      <PageHeader
        title="Docker"
        subtitle="A read-only list of the containers on a server. VPSPilot only lists them: it never starts, stops or changes anything."
      />

      {state.status === "loading" && (
        <p role="status" className="text-sm text-muted">
          Loading servers…
        </p>
      )}
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
            to see its containers here.
          </p>
        </Card>
      )}

      {server && (
        <>
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-56">
              <label htmlFor="docker-server" className="mb-1 block text-sm font-medium">
                Server
              </label>
              <select
                id="docker-server"
                className={inputClass}
                value={server.id}
                onChange={(e) => {
                  setChosen(e.target.value);
                  setQuery("");
                }}
              >
                {servers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            </div>
            <Button
              variant="primary"
              onClick={() => void fetchContainers(server.id)}
              disabled={blocker !== null || load?.busy === true}
            >
              {load?.busy ? "Loading…" : result ? "Refresh" : "Load containers"}
            </Button>
          </div>
          <p className="mt-2 text-xs text-muted">
            Each load signs in once over SSH, runs a single read-only <code>docker ps</code>, and
            disconnects.
          </p>

          {blocker && (
            <p role="status" className="mt-4 rounded-lg bg-warn/10 p-3 text-sm text-warn">
              {blocker}
            </p>
          )}

          {load?.error && (
            <p role="alert" className="mt-4 rounded-lg bg-bad/10 p-3 text-sm text-bad">
              {load.error}
            </p>
          )}

          {result && result.outcome !== "ok" && (
            <Card className="mt-4">
              <p role="alert" className="text-sm text-warn">
                {result.message}
              </p>
            </Card>
          )}

          {result?.outcome === "ok" && (
            <div className="mt-6">
              <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm text-muted">
                  {summary.total} {summary.total === 1 ? "container" : "containers"} ·{" "}
                  {summary.running} running · {summary.stopped} stopped
                  {summary.attention > 0 && (
                    <>
                      {" "}
                      ·{" "}
                      <span className="font-medium text-bad">
                        {summary.attention} need attention
                      </span>
                    </>
                  )}
                  {" · "}updated {new Date(result.fetchedAt).toLocaleTimeString()}
                </p>
                {containers.length > 0 && (
                  <input
                    aria-label="Filter containers"
                    placeholder="Filter by name, image or port"
                    className={`${inputClass} max-w-xs`}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                )}
              </div>

              {result.truncated && (
                <p role="status" className="mb-3 rounded-lg bg-warn/10 p-3 text-sm text-warn">
                  This server has more containers than can be shown. Only the first{" "}
                  {result.containers.length} are listed.
                </p>
              )}

              {containers.length === 0 ? (
                <Card>
                  <p className="text-sm text-muted">There are no containers on this server.</p>
                </Card>
              ) : shown.length === 0 ? (
                <Card>
                  <p className="text-sm text-muted">No containers match that filter.</p>
                </Card>
              ) : (
                <Table headers={["Name", "Image", "State", "Status", "Ports"]}>
                  {shown.map((c) => (
                    <tr key={c.id}>
                      <td className="px-4 py-3">
                        <p className="font-medium">{c.name}</p>
                        <p className="font-mono text-xs text-muted">{c.id}</p>
                      </td>
                      <td className="max-w-56 break-words px-4 py-3 text-muted">{c.image}</td>
                      <td className="whitespace-nowrap px-4 py-3">
                        <div className="flex flex-wrap gap-1">
                          <Badge tone={STATE_TONE[c.state]}>{c.state}</Badge>
                          {c.health && <Badge tone={HEALTH_TONE[c.health]}>{c.health}</Badge>}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-muted">{c.status}</td>
                      <td className="max-w-64 break-words px-4 py-3 text-xs text-muted">
                        {c.ports || "—"}
                      </td>
                    </tr>
                  ))}
                </Table>
              )}
            </div>
          )}
        </>
      )}
    </>
  );
}

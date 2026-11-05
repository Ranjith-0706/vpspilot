import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { Badge, Card, PageHeader } from "../components/ui";
import { Button, inputClass } from "../components/Field";
import { useServers } from "../hooks/useServers";
import { dockerApi } from "../services/dockerApi";
import { sortContainers } from "../services/dockerState";
import { toAppError } from "../services/ipc";
import {
  DEFAULT_TAIL,
  LEVEL_TEXT,
  LEVEL_TONE,
  TAIL_OPTIONS,
  countLevels,
  filterLogLines,
  formatLogTime,
  guessLevel,
  type LevelFilter,
} from "../services/logsState";
import { connectionBlocker } from "../services/serverReadiness";
import { ROUTES } from "../routes";
import type { DockerResult, GuessedLevel, LogsResult } from "../types/docker";

const LEVELS: LevelFilter[] = ["all", "error", "warn", "info", "debug"];

interface ListLoad {
  busy: boolean;
  result?: DockerResult;
  error?: string;
}

interface LogsLoad {
  busy: boolean;
  /** Which server, container and line count `result` belongs to. */
  key: string;
  result?: LogsResult;
  error?: string;
}

export default function LogsPage() {
  const { state } = useServers();
  const [params] = useSearchParams();
  const [chosenServer, setChosenServer] = useState(params.get("server") ?? "");
  const [container, setContainer] = useState(params.get("container") ?? "");
  const containerName = params.get("name") ?? "";
  const [tail, setTail] = useState<number>(DEFAULT_TAIL);
  const [lists, setLists] = useState<Record<string, ListLoad>>({});
  const [logs, setLogs] = useState<LogsLoad | null>(null);
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState<LevelFilter>("all");
  const [wrap, setWrap] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  const servers = state.status === "ready" ? state.servers : [];
  const hostKeys = state.status === "ready" ? state.hostKeys : {};
  const server = servers.find((s) => s.id === chosenServer) ?? servers[0];
  const list = server ? lists[server.id] : undefined;
  const blocker = server ? connectionBlocker(server, hostKeys[server.id]) : null;

  const listed = list?.result?.outcome === "ok" ? sortContainers(list.result.containers) : [];
  const options = listed.map((c) => ({ id: c.id, label: `${c.name} (${c.state})` }));
  // A container reached through a link is usable before the list has been loaded.
  if (container !== "" && !options.some((o) => o.id === container)) {
    options.unshift({ id: container, label: containerName || container });
  }

  const key = `${server?.id ?? ""}|${container}|${tail}`;
  const current = logs?.key === key ? logs : null;
  const result = current?.result;
  const lines = result?.outcome === "ok" ? result.lines : [];
  const shown = filterLogLines(lines, query, level);
  const counts = countLevels(lines);

  // Show the newest lines after each load.
  const fetchedAt = result?.fetchedAt;
  useEffect(() => {
    const el = box.current;
    if (el && fetchedAt !== undefined) el.scrollTop = el.scrollHeight;
  }, [fetchedAt]);

  async function loadContainers(id: string) {
    setLists((prev) => ({ ...prev, [id]: { ...prev[id], busy: true } }));
    try {
      const result = await dockerApi.listContainers(id);
      setLists((prev) => ({ ...prev, [id]: { busy: false, result } }));
    } catch (err) {
      setLists((prev) => ({
        ...prev,
        [id]: { ...prev[id], busy: false, error: toAppError(err).message },
      }));
    }
  }

  async function loadLogs() {
    if (!server || container === "") return;
    const k = key;
    // Keep the last good lines on screen while reloading, but never an old error.
    const keepLast = (prev: LogsLoad | null) =>
      prev?.key === k && prev.result ? { result: prev.result } : {};
    setLogs((prev) => ({ key: k, busy: true, ...keepLast(prev) }));
    try {
      const res = await dockerApi.getLogs(server.id, container, tail);
      setLogs({ key: k, busy: false, result: res });
    } catch (err) {
      setLogs((prev) => ({
        key: k,
        busy: false,
        error: toAppError(err).message,
        ...keepLast(prev),
      }));
    }
  }

  return (
    <>
      <PageHeader
        title="Logs"
        subtitle="Recent output of a Docker container. Read-only: VPSPilot fetches the last lines you choose and never changes anything."
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
            to read its container logs here.
          </p>
        </Card>
      )}

      {server && (
        <>
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-48">
              <label htmlFor="logs-server" className="mb-1 block text-sm font-medium">
                Server
              </label>
              <select
                id="logs-server"
                className={inputClass}
                value={server.id}
                onChange={(e) => {
                  setChosenServer(e.target.value);
                  setContainer("");
                }}
              >
                {servers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            </div>

            <div className="min-w-56">
              <label htmlFor="logs-container" className="mb-1 block text-sm font-medium">
                Container
              </label>
              <select
                id="logs-container"
                className={inputClass}
                value={container}
                onChange={(e) => setContainer(e.target.value)}
              >
                <option value="">
                  {options.length === 0 ? "Load containers first" : "Choose…"}
                </option>
                {options.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>

            <Button
              onClick={() => void loadContainers(server.id)}
              disabled={blocker !== null || list?.busy === true}
            >
              {list?.busy ? "Loading…" : list?.result ? "Reload containers" : "Load containers"}
            </Button>

            <div>
              <label htmlFor="logs-tail" className="mb-1 block text-sm font-medium">
                Lines
              </label>
              <select
                id="logs-tail"
                className={inputClass}
                value={tail}
                onChange={(e) => setTail(Number(e.target.value))}
              >
                {TAIL_OPTIONS.map((n) => (
                  <option key={n} value={n}>
                    Last {n}
                  </option>
                ))}
              </select>
            </div>

            <Button
              variant="primary"
              onClick={() => void loadLogs()}
              disabled={blocker !== null || container === "" || current?.busy === true}
            >
              {current?.busy ? "Loading…" : result ? "Refresh logs" : "Load logs"}
            </Button>
          </div>
          <p className="mt-2 text-xs text-muted">
            Each load signs in once over SSH, runs a single read-only <code>docker</code> command,
            and disconnects.
          </p>

          {blocker && (
            <p role="status" className="mt-4 rounded-md bg-warn/10 p-3 text-sm text-warn">
              {blocker}
            </p>
          )}
          {list?.error && (
            <p role="alert" className="mt-4 rounded-md bg-bad/10 p-3 text-sm text-bad">
              {list.error}
            </p>
          )}
          {list?.result && list.result.outcome !== "ok" && (
            <p role="alert" className="mt-4 rounded-md bg-warn/10 p-3 text-sm text-warn">
              {list.result.message}
            </p>
          )}
          {current?.error && (
            <p role="alert" className="mt-4 rounded-md bg-bad/10 p-3 text-sm text-bad">
              {current.error}
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
                  {lines.length} {lines.length === 1 ? "line" : "lines"}
                  {shown.length !== lines.length && ` (${shown.length} shown)`}
                  {counts.error > 0 && (
                    <>
                      {" · "}
                      <span className="font-medium text-bad">{counts.error} look like errors</span>
                    </>
                  )}
                  {counts.warn > 0 && (
                    <>
                      {" · "}
                      <span className="font-medium text-warn">
                        {counts.warn} look like warnings
                      </span>
                    </>
                  )}
                  {" · "}updated {new Date(result.fetchedAt).toLocaleTimeString()}
                </p>
                <div className="flex flex-wrap items-center gap-3">
                  <input
                    aria-label="Search logs"
                    placeholder="Search the lines"
                    className={`${inputClass} w-56`}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                  <select
                    aria-label="Level (guessed from the text)"
                    title="Docker records no log level, so this is guessed from the text of each line"
                    className={`${inputClass} w-auto`}
                    value={level}
                    onChange={(e) => setLevel(e.target.value as LevelFilter)}
                  >
                    {LEVELS.map((l) => (
                      <option key={l} value={l}>
                        {l === "all" ? "All levels" : `${l} (guessed)`}
                      </option>
                    ))}
                  </select>
                  <label className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={wrap}
                      onChange={(e) => setWrap(e.target.checked)}
                    />
                    Wrap lines
                  </label>
                </div>
              </div>

              {result.truncated && (
                <p role="status" className="mb-3 rounded-md bg-warn/10 p-3 text-sm text-warn">
                  There were more lines than can be shown. Only the newest {lines.length} are
                  listed.
                </p>
              )}

              {lines.length === 0 ? (
                <Card>
                  <p className="text-sm text-muted">This container has not written any logs.</p>
                </Card>
              ) : shown.length === 0 ? (
                <Card>
                  <p className="text-sm text-muted">No lines match those filters.</p>
                </Card>
              ) : (
                <div
                  ref={box}
                  role="log"
                  tabIndex={0}
                  aria-label="Container logs"
                  className="max-h-[calc(100dvh-19rem)] min-h-40 overflow-auto rounded-md border border-border bg-surface py-2 font-mono text-xs"
                >
                  {shown.map((l, i) => {
                    const guessed: GuessedLevel | null = guessLevel(l.message);
                    return (
                      <div key={i} className="flex gap-3 px-3 py-0.5 hover:bg-surface-2">
                        <span className="shrink-0 select-none text-muted">
                          {formatLogTime(l.time)}
                        </span>
                        {guessed && (guessed === "error" || guessed === "warn") && (
                          <span className="shrink-0">
                            <Badge tone={LEVEL_TONE[guessed]}>{guessed}</Badge>
                          </span>
                        )}
                        <span
                          className={`${wrap ? "whitespace-pre-wrap break-words" : "whitespace-pre"} ${guessed ? LEVEL_TEXT[guessed] : ""}`}
                        >
                          {l.message}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}
        </>
      )}
    </>
  );
}

import { useMemo, useState } from "react";
import { Badge, PageHeader, Table } from "../components/ui";
import { logTone } from "../components/tones";
import { mockLogs } from "../demo/demoData";
import type { LogLevel } from "../types";

const LEVELS: ("all" | LogLevel)[] = ["all", "info", "warn", "error", "debug"];

export default function LogsPage() {
  const [level, setLevel] = useState<"all" | LogLevel>("all");
  const [query, setQuery] = useState("");
  const rows = useMemo(
    () =>
      mockLogs.filter(
        (l) =>
          (level === "all" || l.level === level) &&
          `${l.source} ${l.message}`.toLowerCase().includes(query.toLowerCase()),
      ),
    [level, query],
  );
  const field =
    "rounded-lg border border-border bg-surface px-3 py-2 text-sm outline-none focus:border-accent";
  return (
    <>
      <PageHeader title="Logs" subtitle="Filter sample log lines." mock />
      <div className="mb-4 flex flex-wrap gap-3">
        <input
          aria-label="Search logs"
          placeholder="Search source or message…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className={`${field} w-72`}
        />
        <select
          aria-label="Log level"
          value={level}
          onChange={(e) => setLevel(e.target.value as "all" | LogLevel)}
          className={field}
        >
          {LEVELS.map((l) => (
            <option key={l} value={l}>
              {l === "all" ? "All levels" : l}
            </option>
          ))}
        </select>
      </div>
      <Table headers={["Time", "Level", "Source", "Message"]}>
        {rows.length === 0 ? (
          <tr>
            <td colSpan={4} className="px-4 py-6 text-center text-muted">
              No matching log lines.
            </td>
          </tr>
        ) : (
          rows.map((l) => (
            <tr key={l.id} className="font-mono text-xs">
              <td className="px-4 py-2 text-muted">{l.timestamp}</td>
              <td className="px-4 py-2">
                <Badge tone={logTone[l.level]}>{l.level}</Badge>
              </td>
              <td className="px-4 py-2">{l.source}</td>
              <td className="px-4 py-2">{l.message}</td>
            </tr>
          ))
        )}
      </Table>
    </>
  );
}

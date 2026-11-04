import type { Tone } from "../components/tones";
import type { ContainerHealth, ContainerState, DockerContainer } from "../types/docker";

export const STATE_TONE: Record<ContainerState, Tone> = {
  running: "ok",
  restarting: "warn",
  paused: "warn",
  removing: "warn",
  created: "neutral",
  exited: "neutral",
  dead: "bad",
  unknown: "neutral",
};

export const HEALTH_TONE: Record<ContainerHealth, Tone> = {
  healthy: "ok",
  starting: "warn",
  unhealthy: "bad",
};

/** Running first, then the transitional states, then stopped ones. */
const STATE_ORDER: Record<ContainerState, number> = {
  running: 0,
  restarting: 1,
  paused: 2,
  removing: 3,
  created: 4,
  exited: 5,
  dead: 6,
  unknown: 7,
};

export interface ContainerSummary {
  total: number;
  running: number;
  /** Created, exited or dead: present but not running. */
  stopped: number;
  /** Unhealthy, restarting or dead: worth a look. */
  attention: number;
}

export function summarizeContainers(list: DockerContainer[]): ContainerSummary {
  let running = 0;
  let stopped = 0;
  let attention = 0;
  for (const c of list) {
    if (c.state === "running") running++;
    if (c.state === "created" || c.state === "exited" || c.state === "dead") stopped++;
    if (c.health === "unhealthy" || c.state === "restarting" || c.state === "dead") attention++;
  }
  return { total: list.length, running, stopped, attention };
}

/** Sorted by state, then name (case-insensitive). Does not change the input. */
export function sortContainers(list: DockerContainer[]): DockerContainer[] {
  return [...list].sort(
    (a, b) =>
      STATE_ORDER[a.state] - STATE_ORDER[b.state] ||
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
      a.id.localeCompare(b.id),
  );
}

/** Case-insensitive match on name, image, id, status and ports. An empty query keeps everything. */
export function filterContainers(list: DockerContainer[], query: string): DockerContainer[] {
  const q = query.trim().toLowerCase();
  if (q === "") return list;
  return list.filter((c) =>
    [c.name, c.image, c.id, c.status, c.ports].some((field) => field.toLowerCase().includes(q)),
  );
}

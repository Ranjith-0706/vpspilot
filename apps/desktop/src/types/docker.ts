export type ContainerState =
  "running" | "exited" | "restarting" | "paused" | "created" | "dead" | "removing" | "unknown";

export type ContainerHealth = "healthy" | "unhealthy" | "starting";

/** One container as listed by `docker ps -a`. Text fields come from the server and are untrusted. */
export interface DockerContainer {
  /** The short (12 character) id. */
  id: string;
  name: string;
  image: string;
  state: ContainerState;
  health?: ContainerHealth;
  /** Docker's own text, e.g. "Up 3 hours (healthy)". */
  status: string;
  ports: string;
  created: string;
}

export type DockerOutcome =
  | "ok"
  | "not_installed"
  | "permission_denied"
  | "daemon_unavailable"
  /** Logs only: the container no longer exists. */
  | "container_not_found"
  | "error";

export interface DockerResult {
  outcome: DockerOutcome;
  /** Set when the outcome is not "ok". Always written by VPSPilot, never remote text. */
  message?: string;
  containers: DockerContainer[];
  /** More containers existed than are listed. */
  truncated: boolean;
  /** Unix epoch milliseconds. */
  fetchedAt: number;
}

/** One log line. The text is untrusted program output with escape sequences already removed. */
export interface LogLine {
  /** Docker's RFC 3339 timestamp, when the line had one. */
  time?: string;
  message: string;
}

export interface LogsResult {
  outcome: DockerOutcome;
  /** Set when the outcome is not "ok". Always written by VPSPilot, never remote text. */
  message?: string;
  /** Oldest first. */
  lines: LogLine[];
  /** Older lines were dropped to stay within the limit. */
  truncated: boolean;
  /** Unix epoch milliseconds. */
  fetchedAt: number;
}

/** A level worked out from the text of a line. Docker does not record one. */
export type GuessedLevel = "error" | "warn" | "info" | "debug";

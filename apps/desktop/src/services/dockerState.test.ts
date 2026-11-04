import { describe, expect, it } from "vitest";
import {
  HEALTH_TONE,
  STATE_TONE,
  filterContainers,
  sortContainers,
  summarizeContainers,
} from "./dockerState";
import type { ContainerState, DockerContainer } from "../types/docker";

function container(over: Partial<DockerContainer> = {}): DockerContainer {
  return {
    id: "abc123def456",
    name: "web",
    image: "nginx:1.27",
    state: "running",
    status: "Up 3 hours",
    ports: "0.0.0.0:80->80/tcp",
    created: "2026-11-01 10:00:00 +0000 UTC",
    ...over,
  };
}

describe("summarizeContainers", () => {
  it("counts running, stopped and attention separately", () => {
    const s = summarizeContainers([
      container({ state: "running" }),
      container({ state: "running", health: "unhealthy" }),
      container({ state: "exited" }),
      container({ state: "created" }),
      container({ state: "restarting" }),
      container({ state: "dead" }),
      container({ state: "paused" }),
    ]);
    expect(s).toEqual({ total: 7, running: 2, stopped: 3, attention: 3 });
  });

  it("handles an empty list", () => {
    expect(summarizeContainers([])).toEqual({ total: 0, running: 0, stopped: 0, attention: 0 });
  });

  it("does not count a healthy or starting container as needing attention", () => {
    const s = summarizeContainers([
      container({ health: "healthy" }),
      container({ health: "starting" }),
    ]);
    expect(s.attention).toBe(0);
  });
});

describe("sortContainers", () => {
  it("puts running first, then by name ignoring case", () => {
    const sorted = sortContainers([
      container({ name: "zeta", state: "exited", id: "1" }),
      container({ name: "Beta", state: "running", id: "2" }),
      container({ name: "alpha", state: "running", id: "3" }),
      container({ name: "gamma", state: "restarting", id: "4" }),
    ]);
    expect(sorted.map((c) => c.name)).toEqual(["alpha", "Beta", "gamma", "zeta"]);
  });

  it("is stable for equal names by using the id", () => {
    const sorted = sortContainers([container({ id: "b" }), container({ id: "a" })]);
    expect(sorted.map((c) => c.id)).toEqual(["a", "b"]);
  });

  it("does not change its input", () => {
    const input = [container({ name: "b" }), container({ name: "a" })];
    sortContainers(input);
    expect(input.map((c) => c.name)).toEqual(["b", "a"]);
  });
});

describe("filterContainers", () => {
  const list = [
    container({ name: "web", image: "nginx:1.27", id: "aaa111" }),
    container({ name: "db", image: "postgres:16", ports: "5432/tcp", id: "bbb222" }),
    container({ name: "job", image: "alpine", status: "Exited (0) 5 days ago", id: "ccc333" }),
  ];

  it("returns everything for an empty or blank query", () => {
    expect(filterContainers(list, "")).toBe(list);
    expect(filterContainers(list, "   ")).toBe(list);
  });

  it("matches name, image, id, status and ports without regard to case", () => {
    expect(filterContainers(list, "WEB").map((c) => c.name)).toEqual(["web"]);
    expect(filterContainers(list, "postgres").map((c) => c.name)).toEqual(["db"]);
    expect(filterContainers(list, "ccc3").map((c) => c.name)).toEqual(["job"]);
    expect(filterContainers(list, "exited").map((c) => c.name)).toEqual(["job"]);
    expect(filterContainers(list, "5432").map((c) => c.name)).toEqual(["db"]);
  });

  it("treats the query as plain text, not a pattern", () => {
    // As patterns these would match everything; as plain text they match nothing.
    expect(filterContainers(list, ".*")).toEqual([]);
    expect(filterContainers(list, "[a-z]+")).toEqual([]);
    // A literal parenthesis only matches text that really contains one.
    expect(filterContainers(list, "(0)").map((c) => c.name)).toEqual(["job"]);
    expect(filterContainers(list, "[")).toEqual([]);
  });

  it("returns nothing when nothing matches", () => {
    expect(filterContainers(list, "redis")).toEqual([]);
  });
});

describe("tones", () => {
  it("covers every state and health", () => {
    const states: ContainerState[] = [
      "running",
      "exited",
      "restarting",
      "paused",
      "created",
      "dead",
      "removing",
      "unknown",
    ];
    for (const s of states) expect(STATE_TONE[s]).toBeDefined();
    expect(STATE_TONE.running).toBe("ok");
    expect(STATE_TONE.dead).toBe("bad");
    expect(HEALTH_TONE.unhealthy).toBe("bad");
    expect(HEALTH_TONE.healthy).toBe("ok");
  });
});

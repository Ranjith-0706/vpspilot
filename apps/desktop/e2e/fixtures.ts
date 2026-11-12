import { test as base, expect, type Page } from "@playwright/test";
import { installTauriMock, unmockedCommands, type FakeSample, type Scenario } from "./tauriMock";
import type { HostKeyRecord, ServerProfile } from "../src/types/servers";

export const server = (
  id: string,
  name: string,
  patch: Partial<ServerProfile> = {},
): ServerProfile => ({
  id,
  name,
  host: `${id}.example.test`,
  port: 22,
  username: "deploy",
  authMethod: "password",
  credentialRef: `vpspilot:${id}`,
  description: null,
  createdAt: 1,
  updatedAt: 1,
  ...patch,
});

export const trusted = (serverId: string): HostKeyRecord => ({
  serverId,
  keyType: "ssh-ed25519",
  fingerprint: `SHA256:${serverId}-test-fingerprint`,
  trustedAt: 1,
});

/** `count` samples, one a minute, ending `endAgoMin` minutes ago. */
export const samples = (count: number, cpu: number, endAgoMin = 1): FakeSample[] =>
  Array.from({ length: count }, (_, i) => ({ agoMin: endAgoMin + (count - 1 - i), cpu }));

export const ONE_SERVER: Scenario = {
  servers: [server("zionx", "zionx-vps")],
  hostKeys: [trusted("zionx")],
};

export const TWO_SERVERS: Scenario = {
  servers: [server("zionx", "zionx-vps"), server("beta", "beta-vps")],
  hostKeys: [trusted("zionx"), trusted("beta")],
};

/** Opens the app with a scenario. Fails the test on page errors, console errors or unmocked calls. */
export const test = base.extend<{ open: (scenario: Scenario, path?: string) => Promise<Page> }>({
  open: async ({ page }, provide) => {
    const problems: string[] = [];
    page.on("pageerror", (e) => problems.push(`pageerror: ${e.message}`));
    page.on("console", (m) => {
      if (m.type() === "error") problems.push(`console.error: ${m.text()}`);
    });
    await provide(async (scenario, path = "/") => {
      await installTauriMock(page, scenario);
      await page.goto(`/#${path}`);
      return page;
    });
    expect(await unmockedCommands(page).catch(() => []), "commands the mock does not know").toEqual(
      [],
    );
    expect(problems, "errors reported by the page").toEqual([]);
  },
});

export { expect };

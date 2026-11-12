import type { Page } from "@playwright/test";
import { emitMonitor } from "./tauriMock";
import { expect, ONE_SERVER, test, TWO_SERVERS } from "./fixtures";

const SUBTITLE =
  "Manage your VPS servers, monitor system resources, access SSH terminals, and inspect Docker containers.";

const card = (page: Page, name: string) =>
  page.locator("section").filter({ hasText: name }).first();
const row = (page: Page, name: string) => page.getByRole("row").filter({ hasText: name });

async function startMonitoring(page: Page, id: string) {
  await page.getByRole("button", { name: "Start monitoring" }).first().click();
  await emitMonitor(page, id, { type: "status", state: "connecting" });
}

const sample = {
  cpuPercent: 10,
  cores: 2,
  load1: 0.1,
  load5: 0.1,
  load15: 0.1,
  memTotalKib: 1000,
  memAvailableKib: 500,
  memUsedPercent: 50,
  swapTotalKib: 0,
  swapUsedKib: 0,
  diskTotalKib: 1000,
  diskUsedKib: 100,
  diskUsedPercent: 10,
  uptimeSecs: 100,
  sampledAt: Date.now(),
};

async function goOnline(page: Page, id: string) {
  await emitMonitor(page, id, { type: "status", state: "online" });
  await emitMonitor(page, id, { type: "sample", metrics: sample });
}

test.describe("connection status", () => {
  test("Servers page has the new subtitle and no stale text", async ({ open }) => {
    const page = await open(ONE_SERVER, "/servers");
    await expect(page.getByText(SUBTITLE)).toBeVisible();
    await expect(page.getByText(/not implemented yet/)).toHaveCount(0);
  });

  test("a server nobody is connected to is 'Not connected' on both pages", async ({ open }) => {
    const page = await open(ONE_SERVER);
    await expect(card(page, "zionx-vps")).toContainText("Not connected");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Not connected");
    await expect(row(page, "zionx-vps")).toContainText("Not tested");
  });

  test("live monitoring shows as active on Dashboard and on Servers, through navigation", async ({
    open,
  }) => {
    const page = await open(ONE_SERVER);
    await startMonitoring(page, "zionx");
    await expect(card(page, "zionx-vps")).toContainText("Connecting");
    await goOnline(page, "zionx");
    await expect(card(page, "zionx-vps")).toContainText("Monitoring active");

    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Monitoring active");
    await expect(row(page, "zionx-vps")).not.toContainText("Not connected");

    await page.getByRole("link", { name: "Dashboard" }).click();
    await expect(card(page, "zionx-vps")).toContainText("Monitoring active");
  });

  test("a passed connection test does not make a server connected", async ({ open }) => {
    const page = await open(ONE_SERVER, "/servers");
    await page.getByRole("button", { name: "Details zionx-vps" }).click();
    await page.getByRole("button", { name: "Test connection" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Test passed");
    await expect(row(page, "zionx-vps")).toContainText("Not connected");
    // ...and the test result survives leaving the page.
    await page.getByRole("link", { name: "Dashboard" }).click();
    await expect(card(page, "zionx-vps")).toContainText("Not connected");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Test passed");
  });

  test("a stale passed test does not override active monitoring", async ({ open }) => {
    const page = await open(ONE_SERVER, "/servers");
    await page.getByRole("button", { name: "Details zionx-vps" }).click();
    await page.getByRole("button", { name: "Test connection" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Test passed");
    await page.getByRole("link", { name: "Dashboard" }).click();
    await startMonitoring(page, "zionx");
    await goOnline(page, "zionx");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Monitoring active");
    await expect(row(page, "zionx-vps")).toContainText("Test passed");
  });

  test("a network drop shows Reconnecting, and recovery shows Monitoring active again", async ({
    open,
  }) => {
    const page = await open(ONE_SERVER);
    await startMonitoring(page, "zionx");
    await goOnline(page, "zionx");
    await emitMonitor(page, "zionx", {
      type: "status",
      state: "retrying",
      message: "Could not reach zionx.example.test:22",
      cause: "unreachable",
    });
    await expect(card(page, "zionx-vps")).toContainText("Reconnecting");
    await expect(card(page, "zionx-vps")).not.toContainText("Monitoring active");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Reconnecting");

    await page.getByRole("link", { name: "Dashboard" }).click();
    await emitMonitor(page, "zionx", { type: "status", state: "connecting" });
    await goOnline(page, "zionx");
    await expect(card(page, "zionx-vps")).toContainText("Monitoring active");
  });

  test("an authentication failure is never shown as online", async ({ open }) => {
    const page = await open(ONE_SERVER);
    await startMonitoring(page, "zionx");
    await emitMonitor(page, "zionx", {
      type: "status",
      state: "failed",
      message: "The server rejected the username or credential.",
      cause: "auth_failed",
    });
    await expect(card(page, "zionx-vps")).toContainText("Authentication failed");
    await expect(card(page, "zionx-vps")).not.toContainText("Monitoring active");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Authentication failed");
  });

  test("other failures are a connection error, not an authentication failure", async ({ open }) => {
    const page = await open(ONE_SERVER);
    await startMonitoring(page, "zionx");
    await emitMonitor(page, "zionx", {
      type: "status",
      state: "failed",
      message: "WARNING: the server's host key does not match.",
      cause: "host_key_changed",
    });
    await expect(card(page, "zionx-vps")).toContainText("Connection error");
    await expect(card(page, "zionx-vps")).not.toContainText("Authentication failed");
  });

  test("stopping monitoring leaves no active status", async ({ open }) => {
    const page = await open(ONE_SERVER);
    await startMonitoring(page, "zionx");
    await goOnline(page, "zionx");
    await expect(card(page, "zionx-vps")).toContainText("Monitoring active");
    await page.getByRole("button", { name: "Stop monitoring" }).click();
    await expect(card(page, "zionx-vps")).toContainText("Not connected");
    await expect(card(page, "zionx-vps")).not.toContainText("Monitoring active");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Not connected");
  });

  test("each of several servers keeps its own status", async ({ open }) => {
    const page = await open(TWO_SERVERS);
    await page
      .locator("section")
      .filter({ hasText: "zionx-vps" })
      .getByRole("button", { name: "Start monitoring" })
      .first()
      .click();
    await goOnline(page, "zionx");
    await page.getByRole("link", { name: "Servers" }).click();
    await expect(row(page, "zionx-vps")).toContainText("Monitoring active");
    await expect(row(page, "beta-vps")).toContainText("Not connected");
  });
});

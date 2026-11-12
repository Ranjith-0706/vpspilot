import { callsTo } from "./tauriMock";
import { expect, ONE_SERVER, samples, server, test, trusted, TWO_SERVERS } from "./fixtures";

test.describe("History tab", () => {
  test("shows recorded history, clearly labelled as not current status", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, history: { zionx: samples(30, 42, 5) } });
    await page.getByRole("tab", { name: "History" }).click();

    await expect(page.getByRole("note")).toContainText("Historical data");
    await expect(page.getByRole("note")).toContainText("not the current status");
    for (const title of ["CPU history", "Memory history", "Disk history", "Load average history"]) {
      await expect(page.getByRole("figure").filter({ hasText: title })).toBeVisible();
    }
    await expect(page.getByText("30 samples recorded")).toBeVisible();
    await expect(page.getByText(/latest was 5 minutes ago/)).toBeVisible();

    // History must never present itself as live status.
    const panel = page.getByRole("tabpanel");
    await expect(panel).not.toContainText("Monitoring active");
    await expect(panel).not.toContainText("Connected");
  });

  test("the Live tab is unchanged and can be returned to", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, history: { zionx: samples(5, 10) } });
    await expect(page.getByRole("tab", { name: "Live" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("button", { name: "Start monitoring" })).toBeVisible();
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByRole("button", { name: "Start monitoring" })).toHaveCount(0);
    await page.getByRole("tab", { name: "Live" }).click();
    await expect(page.getByRole("button", { name: "Start monitoring" })).toBeVisible();
  });

  test("an empty range says so instead of drawing a chart", async ({ open }) => {
    const page = await open(ONE_SERVER);
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByText(/No history recorded in this time range/)).toBeVisible();
    await expect(page.getByText("Nothing has been recorded")).toBeVisible();
    await expect(page.getByRole("figure")).toHaveCount(0);
  });

  test("a single point is flagged as too little for a trend", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, history: { zionx: samples(1, 10) } });
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByText(/Only one point has been recorded/)).toBeVisible();
  });

  test("a gap in collection is drawn as a break, not a line across it", async ({ open }) => {
    const page = await open({
      ...ONE_SERVER,
      history: { zionx: [...samples(10, 30, 50), ...samples(10, 30, 5)] }, // 35 minutes missing
    });
    await page.getByRole("tab", { name: "History" }).click();
    const cpu = page.getByRole("figure").filter({ hasText: "CPU history" });
    await expect(cpu.locator("polyline")).toHaveCount(2);
  });

  test("a failed load shows an error, not stale or empty data", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, historyError: "The database is unavailable." });
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByRole("alert")).toContainText("History could not be loaded");
    await expect(page.getByRole("figure")).toHaveCount(0);
  });

  test("changing the range asks for that range", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, history: { zionx: samples(20, 10) } });
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByText("20 samples recorded")).toBeVisible();
    await page.getByLabel("Time range").selectOption({ label: "Last 24 hours" });
    await expect
      .poll(async () => (await callsTo(page, "get_metric_history")).map((c) => c.args.rangeSecs))
      .toContain(86400);
  });

  test("only ranges the retention can fill are offered", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, retentionDays: 1 });
    await page.getByRole("tab", { name: "History" }).click();
    const options = page.getByLabel("Time range").locator("option");
    await expect(options).toHaveText(["Last hour", "Last 6 hours", "Last 24 hours"]);
  });

  test("each server keeps its own history, even when replies arrive late", async ({ open }) => {
    const page = await open({
      ...TWO_SERVERS,
      history: { zionx: samples(30, 10), beta: samples(7, 90) },
      historyDelayMs: { zionx: 700 }, // the first server answers slowly
    });
    await page.getByRole("tab", { name: "History" }).click();
    // Switch away before the slow reply for zionx arrives.
    await page.getByLabel("Server", { exact: true }).selectOption({ label: "beta-vps" });
    await expect(page.getByText("7 samples recorded")).toBeVisible();
    await page.waitForTimeout(1000); // the late zionx reply has now arrived and must be ignored
    await expect(page.getByText("7 samples recorded")).toBeVisible();
    await expect(page.getByText("30 samples recorded")).toHaveCount(0);

    await page.getByLabel("Server", { exact: true }).selectOption({ label: "zionx-vps" });
    await expect(page.getByText("30 samples recorded")).toBeVisible();
    await expect(page.getByText("7 samples recorded")).toHaveCount(0);
  });

  test("a server without history is not shown another server's data", async ({ open }) => {
    const page = await open({
      servers: [server("a", "a-vps"), server("b", "b-vps")],
      hostKeys: [trusted("a"), trusted("b")],
      history: { a: samples(12, 10) },
    });
    await page.getByRole("tab", { name: "History" }).click();
    await expect(page.getByText("12 samples recorded")).toBeVisible();
    await page.getByLabel("Server", { exact: true }).selectOption({ label: "b-vps" });
    await expect(page.getByText(/No history recorded/)).toBeVisible();
    await expect(page.getByText("12 samples recorded")).toHaveCount(0);
  });
});

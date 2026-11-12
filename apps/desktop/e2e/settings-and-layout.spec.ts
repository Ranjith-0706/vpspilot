import { callsTo } from "./tauriMock";
import { expect, ONE_SERVER, samples, test } from "./fixtures";

test.describe("history retention setting", () => {
  test("loads the saved value and saves a valid change", async ({ open }) => {
    const page = await open({ ...ONE_SERVER, retentionDays: 14 }, "/settings");
    const input = page.getByLabel("Keep history for (days)");
    await expect(input).toHaveValue("14");
    await expect(page.getByRole("button", { name: "Save" })).toBeDisabled(); // unchanged

    await input.fill("30");
    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByRole("status")).toContainText("Saved");
    expect((await callsTo(page, "set_history_retention_days")).map((c) => c.args.days)).toEqual([
      30,
    ]);
  });

  test("rejects values outside 1 to 90 without calling the backend", async ({ open }) => {
    const page = await open(ONE_SERVER, "/settings");
    const input = page.getByLabel("Keep history for (days)");
    for (const bad of ["0", "91", "abc", "1.5", ""]) {
      await input.fill(bad);
      await expect(page.getByRole("alert")).toContainText("whole number from 1 to 90");
      await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
    }
    expect(await callsTo(page, "set_history_retention_days")).toEqual([]);
  });
});

const VIEWPORTS = [
  { name: "wide", width: 1440, height: 900 },
  { name: "narrow", width: 700, height: 900 },
];
const PAGES = [
  { name: "dashboard-live", path: "/", tab: null },
  { name: "dashboard-history", path: "/", tab: "History" },
  { name: "servers", path: "/servers", tab: null },
  { name: "settings", path: "/settings", tab: null },
];

for (const theme of ["light", "dark"] as const) {
  for (const vp of VIEWPORTS) {
    test.describe(`layout: ${theme}, ${vp.name}`, () => {
      test.use({ viewport: { width: vp.width, height: vp.height }, colorScheme: theme });

      for (const p of PAGES) {
        test(`${p.name} fits the window`, async ({ open }, testInfo) => {
          const page = await open(
            { ...ONE_SERVER, history: { zionx: samples(40, 35, 3) } },
            p.path,
          );
          if (p.tab) await page.getByRole("tab", { name: p.tab }).click();
          await page.waitForLoadState("networkidle");
          await expect(page.locator("main, #root").first()).toBeVisible();
          if (p.tab) await expect(page.getByRole("figure").first()).toBeVisible();

          // No horizontal scrolling at this width.
          const overflow = await page.evaluate(
            () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
          );
          expect(overflow, "horizontal overflow in px").toBeLessThanOrEqual(0);

          // Keep a picture for a human to look at; this test does not compare images.
          await testInfo.attach(`${p.name}-${theme}-${vp.name}`, {
            body: await page.screenshot({ fullPage: true }),
            contentType: "image/png",
          });
        });
      }
    });
  }
}

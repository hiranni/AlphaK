import fs from "node:fs";
import { expect, test } from "@playwright/test";

test("late stock responses cannot overwrite the current symbol", async ({ page }) => {
  await page.route(/\/api\/v1\/instruments\/600519\.SH$/, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 700));
    await route.continue();
  });
  await page.route(/\/api\/v1\/instruments\/600519\.SH\/fundamentals$/, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 700));
    await route.continue();
  });
  await page.goto("/stock/600519.SH");
  await page.evaluate(() => {
    history.pushState({}, "", "/stock/000858.SZ");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page.locator("h1")).toContainText("五粮液");
  await page.waitForTimeout(900);
  await expect(page.locator("h1")).toContainText("五粮液");
});

test("chart retry issues a new bars request", async ({ page }) => {
  let failures = 2;
  await page.route("**/bars*", async (route) => {
    if (failures > 0) {
      failures -= 1;
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ code: 1, message: "TEST_BARS_FAILURE", data: null, request_id: "test" }),
      });
    } else {
      await route.continue();
    }
  });
  await page.goto("/stock/600519.SH");
  await expect(page.getByRole("button", { name: "重试" })).toBeVisible();
  await page.getByRole("button", { name: "重试" }).click();
  await expect(page.locator(".chart-main canvas").first()).toBeVisible();
});

test("default home and density settings are applied on load", async ({ page }) => {
  await page.request.put("/api/v1/settings", {
    data: { default_home: "/watchlist", density: "compact" },
  });
  await page.goto("/");
  await expect(page).toHaveURL(/\/watchlist$/);
  await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
  await page.goto("/stock/600519.SH");
  await expect(page).toHaveURL(/\/stock\/600519\.SH$/);
  await page.request.put("/api/v1/settings", {
    data: { default_home: "/", density: "comfortable" },
  });
});

test("CSV export/import round-trips commas, quotes, and newlines", async ({ page }) => {
  const note = '长期,关注"重点"\n二行';
  await page.request.post("/api/v1/watchlists/items", {
    data: { group_id: "grp-default", symbol: "600900.SH", note },
  });
  await page.goto("/watchlist");
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出" }).click();
  const download = await downloadPromise;
  const path = await download.path();
  expect(path).toBeTruthy();
  const csv = fs.readFileSync(path!, "utf8");
  expect(csv).toContain('"长期,关注""重点""\n二行"');

  await page.getByRole("button", { name: "新建分组" }).click();
  await page.getByPlaceholder("新分组名称").fill("CSV 往返");
  await page.getByRole("button", { name: "创建" }).click();
  await page.locator('input[type="file"]').setInputFiles(path!);
  await expect(page.locator(".inline-notice")).toContainText("导入完成");
  await expect(page.locator(".watch-main")).toContainText('长期,关注"重点"');
});

test("main chart and indicator keep the same visible time range", async ({ page }) => {
  await page.goto("/stock/600519.SH");
  await expect(page.locator(".chart-sub")).toBeVisible();
  const main = page.locator(".chart-main");
  const sub = page.locator(".chart-sub");
  await expect(main).toHaveAttribute("data-visible-range", /.+/);
  await expect(sub).toHaveAttribute("data-visible-range", /.+/);
  expect(await main.getAttribute("data-visible-range")).toBe(await sub.getAttribute("data-visible-range"));

  const box = await main.boundingBox();
  expect(box).toBeTruthy();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.mouse.move(box!.x + box!.width / 2 - 120, box!.y + box!.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect
    .poll(async () => {
      const mainRange = await main.getAttribute("data-visible-range");
      const subRange = await sub.getAttribute("data-visible-range");
      return mainRange === subRange;
    })
    .toBe(true);
});

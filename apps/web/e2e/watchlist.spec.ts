import { expect, test } from "@playwright/test";

test("name, code and pinyin search add a stock and group deletion removes its membership", async ({ page, request }) => {
  const name = `搜索删除验证-${Date.now()}`;
  let groupId = "";
  try {
    await page.goto("/watchlist");
    await page.getByRole("button", { name: "新建分组" }).click();
    await page.getByPlaceholder("新分组名称").fill(name);
    await page.getByRole("button", { name: "创建", exact: true }).click();
    await expect(page.locator(".watch-main h2")).toHaveText(name);
    const response = await request.get("/api/v1/watchlists");
    const groups = await response.json();
    groupId = groups.data.find((group: { name: string }) => group.name === name).id;
    const input = page.getByLabel("证券代码", { exact: true });
    const result = page.getByRole("button", { name: "贵州茅台 · 600519.SH · 加入", exact: true });
    for (const query of ["gzmt", "600519", "贵州茅台"]) {
      await input.fill(query);
      await expect(result).toBeVisible();
    }
    await result.click();
    await expect(page.locator(".watch-table")).toContainText("600519.SH");
    await page.getByRole("button", { name: "删除当前分组", exact: true }).click();
    await expect(page.getByRole("alertdialog")).toContainText(name);
    await page.getByRole("button", { name: "取消", exact: true }).click();
    await expect(page.locator(".watch-main h2")).toHaveText(name);
    await page.getByRole("button", { name: "删除当前分组", exact: true }).click();
    await page.getByRole("button", { name: "确认删除", exact: true }).click();
    await expect(page.locator(".watch-main h2")).not.toHaveText(name);
    const after = await (await request.get("/api/v1/watchlists")).json();
    expect(after.data.some((group: { id: string }) => group.id === groupId)).toBe(false);
    groupId = "";
  } finally {
    if (groupId) await request.delete(`/api/v1/watchlists/groups/${groupId}`);
  }
});

test("watchlist add, import, refresh, and drag order", async ({ page }) => {
  await page.goto("/watchlist");
  const symbol = page.locator('.add-row input[aria-label="证券代码"]');
  await symbol.fill("000333.SZ");
  await page.getByRole("button", { name: "加入" }).click();
  await expect(page.locator(".watch-main")).toContainText("美的集团");

  await page.locator('input[type="file"]').setInputFiles({
    name: "watch-e2e.csv",
    mimeType: "text/csv",
    buffer: Buffer.from("symbol,name,note\n601899.SH,紫金矿业,e2e导入\n"),
  });
  await expect(page.locator(".inline-notice")).toContainText("导入完成");

  await page.getByRole("button", { name: "刷新" }).click();
  await expect(page.locator(".inline-notice")).toContainText("已请求更新");

  const rows = page.locator(".watch-table tbody tr");
  const firstBefore = await rows.first().innerText();
  const secondBefore = await rows.nth(1).innerText();
  await rows.first().dragTo(rows.nth(1));
  await expect(rows.first()).not.toHaveText(firstBefore);
  await expect(page.locator(".watch-table tbody")).toContainText(secondBefore.split("\n")[0]);
});

test("watchlist column preferences persist", async ({ page }) => {
  await page.goto("/watchlist");
  await page.locator(".field-menu summary").click();
  await page.getByLabel("最新价").uncheck();
  await page.reload();
  await expect(page.locator(".watch-table thead")).not.toContainText("最新价");
  await page.evaluate(() => localStorage.removeItem("alphak.watch.fields"));
});

test("500-row watchlist renders only the visible window", async ({ page }) => {
  const items = Array.from({ length: 500 }, (_, index) => ({
    id: `virtual-${index}`,
    symbol: "600519.SH",
    name: `虚拟证券${index}`,
    note: "",
    sort: index,
    close: 100 + index,
    change_pct: 0,
    as_of: "2026-10-21",
  }));
  await page.route("**/api/v1/watchlists", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        code: 0,
        message: "ok",
        data: [{ id: "grp-virtual", name: "500 只测试", sort: 0, items }],
        request_id: "virtual-test",
      }),
    }),
  );
  await page.goto("/watchlist");
  await expect(page.locator(".virtual-table")).toBeVisible();
  const rendered = await page.locator("[data-row]").count();
  expect(rendered).toBeGreaterThan(0);
  expect(rendered).toBeLessThan(100);
  await page.locator(".virtual-table").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(page.locator(".watch-table tbody")).toContainText("虚拟证券499");
});

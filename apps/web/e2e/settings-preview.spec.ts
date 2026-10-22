import { expect, test } from "@playwright/test";

test("settings apply and save automatically without save or cancel buttons", async ({ page }) => {
  let saved = { theme: "dark", up_color: "red", density: "comfortable", default_home: "/", timezone: "Asia/Shanghai", poll_interval: 30 };
  let writes = 0;
  await page.route("**/api/v1/settings", async route => {
    if (route.request().method() === "PUT") { saved = { ...saved, ...route.request().postDataJSON() }; writes += 1; }
    await route.fulfill({ json: { code: 0, message: "ok", data: saved, request_id: "settings-test" } });
  });
  await page.goto("/settings");
  await page.getByLabel("主题", { exact: true }).selectOption("light");
  await page.getByLabel("涨跌颜色", { exact: true }).selectOption("green");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(page.locator("html")).toHaveAttribute("data-up-color", "green");
  await expect.poll(() => writes).toBe(2);
  await expect(page.getByRole("button", { name: /保存设置|取消更改/ })).toHaveCount(0);
  await page.locator('.rail-link[href="/"]').click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.goto("/settings");
  await expect(page.getByLabel("涨跌颜色", { exact: true })).toHaveValue("green");
});

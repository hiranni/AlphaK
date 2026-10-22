import { expect, test } from "@playwright/test";

test("search a symbol, open research tabs, and switch chart controls", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+K");
  await page.locator(".palette-input input").fill("gzmt");
  await expect(page.locator(".palette-row").first()).toContainText("贵州茅台");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/stock\/600519\.SH$/);
  await expect(page.locator("h1")).toContainText("贵州茅台");

  await page.getByRole("tab", { name: "财务" }).click();
  await expect(page.locator(".financial-panel")).toContainText("财务快照");
  await page.getByRole("tab", { name: "公司资料" }).click();
  await expect(page.locator(".profile-panel")).toContainText("公司资料");

  await page.getByRole("tab", { name: "概览" }).click();
  await page.getByRole("button", { name: "周" }).click();
  await page.getByRole("button", { name: "后复权" }).click();
  await page.getByRole("button", { name: "KDJ" }).click();
  await expect(page.locator(".chart-sub")).toBeVisible();
});

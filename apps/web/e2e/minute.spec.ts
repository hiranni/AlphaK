import { expect, test } from "@playwright/test";

test("minute candles render, real supplements fill panels and refresh after sixty seconds", async ({ page }) => {
  await page.clock.install();
  const envelope = (data: unknown) => ({ code: 0, message: "ok", data, request_id: "minute-test" });
  let minuteRequests = 0;
  await page.route("**/api/v1/instruments/600519.SH/bars?**", async route => {
    const period = new URL(route.request().url()).searchParams.get("period") ?? "daily";
    if (period === "5m") minuteRequests += 1;
    const bars = Array.from({ length: 65 }, (_, index) => ({ date: period.endsWith("m") ? String(1792632900 + index * 300) : `2026-${index < 30 ? "08" : index < 60 ? "09" : "10"}-${String(index % 30 + 1).padStart(2, "0")}`, open: 10, high: 12, low: 9, close: 11, volume: 100, amount: 1100, ma5: 10, ma10: null, ma20: 10, ma60: null, vol_ma5: null, vol_ma10: null, ema12: null, boll_upper: null, boll_mid: null, boll_lower: null, dif: null, dea: null, macd: null, k: null, d: null, j: null, rsi: null }));
    await route.fulfill({ json: envelope({ symbol: "600519.SH", name: "贵州茅台", period, adjust: "none", adjust_available: false, notice: "sina · 每60秒更新", as_of: bars.at(-1)?.date, bars }) });
  });
  await page.route("**/api/v1/instruments/600519.SH/supplement/*", async route => {
    const dataset = route.request().url().split("/").at(-1);
    const data = dataset === "metrics" ? { total_mv: 1645000000000, float_mv: 1645000000000, pe_ttm: 18.48, pb: 6.55, turnover: 0.2, total_shares: 1250000000, float_shares: 1250000000 } : dataset === "profile" ? { ORG_NAME: "贵州茅台酒股份有限公司", SECURITY_NAME_ABBR: "贵州茅台", BOARD_NAME_1LEVEL: "食品饮料", LISTING_DATE: "2001-08-27", ORG_WEB: "www.moutaichina.com", MAIN_BUSINESS: "白酒生产与销售", ORG_PROFIE: "公司资料验证", REAL_CONTROLER: "贵州省国资委" } : [{ REPORT_DATE: "2026-06-30", NOTICE_DATE: "2026-08-15", TOTAL_OPERATE_INCOME: 92278072083, PARENT_NETPROFIT: 44516880421, TOTAL_ASSETS: 309050784569, TOTAL_LIABILITIES: 46954432394, TOTAL_EQUITY: 262096352174, NETCASH_OPERATE: 70690750119, ROEJQ: 16.75, XSMLL: 89.55 }];
    await route.fulfill({ json: envelope({ data, source: "eastmoney", updated_at: 1792632900, stale: false, error: "", retry_count: 0 }) });
  });
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/stock/600519.SH");
  await page.getByRole("button", { name: "5分", exact: true }).click();
  await expect(page.locator(".chart-legend")).toContainText("5分钟");
  await expect(page.locator(".stock-stats")).toContainText("18.48 / 6.55");
  await expect(page.locator(".chart-main canvas").first()).toBeVisible();
  const before = minuteRequests;
  await page.clock.fastForward(61000);
  await expect.poll(() => minuteRequests).toBeGreaterThan(before);
  await page.getByRole("tab", { name: "财务", exact: true }).click();
  await expect(page.locator(".financial-grid")).toContainText("16.75%");
  await expect(page.locator(".financial-grid")).toContainText("706.91 亿");
  await page.getByRole("tab", { name: "公司资料", exact: true }).click();
  await expect(page.locator(".profile-copy")).toContainText("贵州茅台酒股份有限公司");
  await expect(page.locator('.profile-facts a')).toHaveAttribute("href", "https://www.moutaichina.com");
  expect(errors).toEqual([]);
});

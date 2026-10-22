import { expect, test } from "@playwright/test";

test("market heat filters and panels stay inside the viewport", async ({ page }) => {
  await page.route("**/api/v1/market/overview*", route => route.fulfill({ json: { code: 0, data: {
    as_of: "2026-10-22", source: "test", indices: Array.from({ length: 5 }, (_, index) => ({ code: String(index), name: `指数${index}`, close: 3500, change_points: 12, change_pct: 1, amount: 100, amount_known: true })),
    breadth: { advance: 60, decline: 40, flat: 0, limit_up: 1, limit_down: 1, total_amount: 100, amount_known: true },
    sectors: Array.from({ length: 49 }, (_, index) => ({ id: -index - 1, name: `行业${index}`, change_pct: 24 - index, turnover: 100, sector_type: "industry" })),
  } } }));
  await page.route("**/api/v1/market/popularity", route => route.fulfill({ json: { code: 0, data: { data: Array.from({ length: 12 }, (_, index) => ({ symbol: `00000${index}.SZ`, name: `人气${index}`, rank: index + 1, close: 10, change_pct: 1 })), source: "eastmoney", updated_at: 1792600000, stale: false } } }));
  await page.route("**/api/v1/market/news*", route => route.fulfill({ json: { code: 0, data: { data: Array.from({ length: 6 }, (_, index) => ({ id: `news-${index}`, title: `热点新闻 ${index}`, url: "https://finance.sina.com.cn/", source: "证券时报", published_at: 1792600000 + index, summary: "" })), source: "sina", updated_at: 1792600000, stale: false } } }));
  await page.goto("/");
  const rows = page.locator(".sectors-panel tbody tr");
  await expect(rows).toHaveCount(8);
  await expect(rows.last()).toContainText("行业7");
  await expect(page.getByRole("button", { name: "正常", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "热度最低", exact: true }).click();
  await expect(rows.first()).toContainText("行业48");
  await page.getByRole("button", { name: "热度最高", exact: true }).click();
  await expect(rows.last()).toContainText("行业7");
  await expect(page.locator(".watch-row")).toHaveCount(12);
  await expect(page.locator(".news-row")).toHaveCount(6);
  await expect(page.locator(".news-panel")).toBeVisible();
  for (const [width, height] of [[1440, 900], [1366, 768], [1280, 720], [1024, 768], [768, 1024], [375, 812]]) {
    await page.setViewportSize({ width, height });
    const dimensions = await page.evaluate(() => ({ height: document.documentElement.scrollHeight, width: document.documentElement.scrollWidth }));
    expect(dimensions.height, `${width}x${height} height`).toBeLessThanOrEqual(height + 1);
    expect(dimensions.width, `${width}x${height} width`).toBeLessThanOrEqual(width + 1);
    await expect(page.locator(".sectors-panel")).toBeVisible();
    await expect(page.locator(".watch-panel")).toBeVisible();
    await expect(page.locator(".news-panel")).toBeVisible();
    if (width >= 768) {
      const edges = await page.evaluate(() => {
        const tickers = [...document.querySelectorAll<HTMLElement>(".ticker")];
        const watch = document.querySelector<HTMLElement>(".watch-panel");
        if (!watch || tickers.length < 5) return null;
        const round = (value: number) => Math.round(value * 10) / 10;
        return {
          watchLeft: round(watch.getBoundingClientRect().left),
          watchRight: round(watch.getBoundingClientRect().right),
          fourthLeft: round(tickers[3].getBoundingClientRect().left),
          lastRight: round(tickers[tickers.length - 1].getBoundingClientRect().right),
        };
      });
      expect(edges, `${width}x${height} edges`).not.toBeNull();
      expect(edges!.watchLeft, `${width}x${height} watch left aligns with fourth index card`).toBeCloseTo(edges!.fourthLeft, 0);
      expect(edges!.watchRight, `${width}x${height} watch right aligns with last index card`).toBeCloseTo(edges!.lastRight, 0);
    }
  }
});

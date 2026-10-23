import { expect, test } from '@playwright/test';

test('daily workbench keeps unavailable analysis explicit and opens keyboard-accessible quick analysis', async ({ page }) => {
  await page.route('**/api/v1/watchlists/workbench', async route => {
    const response = await route.fetch();
    const body = await response.json();
    body.data.analyses = body.data.analyses.map((a: Record<string, unknown>) => ({ ...a, status: 'missing', close: null, change: null, change_pct: null, score: null, signals: [], sparkline: [], history_start: '', notice: '测试：日线数据不足' }));
    await route.fulfill({ response, json: body });
  });
  await page.goto('/watchlist');
  await expect(page.getByRole('heading', { name: '自选工作台', exact: true })).toBeVisible();
  await expect(page.locator('.watch-table')).not.toContainText('/100');
  const stock = page.locator('.watch-stock-button').first();
  await stock.click();
  const dialog = page.getByRole('dialog', { name: '个股快捷分析' });
  await expect(dialog).toContainText('测试：日线数据不足');
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
});

test('responsive workbench contains horizontal scrolling within the table', async ({ page }) => {
  await page.goto('/watchlist');
  await expect(page.getByRole('heading', { name: '自选工作台', exact: true })).toBeVisible();
  for (const width of [320, 375, 414, 768]) {
    await page.setViewportSize({ width, height: 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1)).toBe(true);
  }
});

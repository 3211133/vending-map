import { expect, test } from '@playwright/test';

test('地図が表示される', async ({ page }) => {
  await page.goto('/');

  await expect(page.locator('#map')).toBeVisible();
  await expect(page).toHaveTitle(/vending-map/i);
});

test('サイドバーのボタンが表示される', async ({ page }) => {
  await page.goto('/');

  await expect(page.locator('#toggleAddModeButton')).toBeVisible();
  await expect(page.locator('#locateButton')).toBeVisible();
});

test('自販機データが読み込まれる', async ({ page }) => {
  await page.goto('/');

  const statusMessage = page.locator('#statusMessage');
  await expect(statusMessage).not.toHaveText('地図を読み込み中...', { timeout: 10000 });
  await expect(statusMessage).toContainText('件を表示中');
});

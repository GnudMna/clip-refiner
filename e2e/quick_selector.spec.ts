import { test, expect } from './fixtures';

test.use({ e2eWindow: 'quick' });

test.describe('クイックセレクター WebView', () => {
  test('検索入力と結果一覧が表示される', async ({ page }) => {
    await expect(page.locator('#input')).toBeVisible();
    await expect(page.locator('#results')).toBeVisible();
    await expect(page.locator('#results .item').first()).toBeVisible();
  });

  test('検索でモードを絞り込める', async ({ page }) => {
    const input = page.locator('#input');
    await input.fill('trim');
    const items = page.locator('#results .item');
    await expect(items.first()).toBeVisible();
    expect(await items.count()).toBeGreaterThan(0);
    await expect(items.first()).toContainText(/トリム|trim/i);
  });

  test('お気に入りが先頭付近に並ぶ', async ({ page }) => {
    const first = page.locator('#results .item').first();
    await expect(first).toBeVisible();
    // お気に入り先頭 (Trim) は日本語ラベル + ★
    await expect(first).toContainText(/トリム/);
    await expect(first.locator('.favorite-button.is-favorite')).toBeVisible();
  });

  test('Escape でセレクターが閉じる', async ({ page }) => {
    await expect(page.locator('#container')).toBeVisible();
    const closed = page.waitForEvent('close', { timeout: 15_000 });
    // WebView 破棄で CDP が切れると press が失敗することがある
    await Promise.all([
      closed,
      page.keyboard.press('Escape').catch(() => undefined),
    ]);
    expect(page.isClosed()).toBe(true);
  });
});

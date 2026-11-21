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

  test('矢印キーで選択行が移動し Enter で閉じる', async ({ page }) => {
    const items = page.locator('#results .item');
    await expect(items.first()).toBeVisible();
    await page.locator('#input').click();
    // 起動時は現在モード行が選択されていることがある
    await page.keyboard.press('Home');
    await expect(items.nth(0)).toHaveClass(/selected/);

    await page.keyboard.press('ArrowDown');
    await expect(items.nth(1)).toHaveClass(/selected/);
    await expect(items.nth(0)).not.toHaveClass(/selected/);

    const closed = page.waitForEvent('close', { timeout: 15_000 });
    await Promise.all([
      closed,
      page.keyboard.press('Enter').catch(() => undefined),
    ]);
    expect(page.isClosed()).toBe(true);
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

test.describe('クイックセレクター お気に入り並び', () => {
  test.use({
    e2eWindow: 'quick',
    harnessOptions: { favoriteModes: ['UrlDecode', 'Trim'] },
  });

  test('config のお気に入り順が先頭行に反映される', async ({ page }) => {
    const first = page.locator('#results .item').first();
    await expect(first).toBeVisible();
    // UrlDecode が favoriteIndex 0
    await expect(first).toContainText('URLデコード');
    await expect(first.locator('.favorite-button.is-favorite')).toBeVisible();
  });
});

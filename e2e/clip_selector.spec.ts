import { test, expect } from './fixtures';

test.use({ e2eWindow: 'clip' });

test.describe('登録クリップセレクター WebView', () => {
  test('検索入力と結果領域が表示される', async ({ page }) => {
    await expect(page.locator('#input')).toBeVisible();
    await expect(page.locator('#results')).toBeVisible();
    await expect(page.locator('#container')).toBeVisible();
  });

  test('空のときは空メッセージと登録行が表示される', async ({ page }) => {
    await expect(page.locator('#results .empty')).toHaveText('登録クリップがありません');
    await expect(page.locator('#results .item.action')).toContainText('クリップボードを登録');
  });

  test('登録クリップを検索で絞り込める', async ({ page }) => {
    await page.evaluate(() => {
      (
        window as unknown as {
          refreshItems: (
            items: Array<{ id: string; label: string; preview?: string; kind: string }>,
          ) => void;
        }
      ).refreshItems([
        { id: '0', label: 'サンプルテキスト', preview: 'hello world', kind: 'text' },
        { id: '1', label: '別のクリップ', preview: 'other value', kind: 'text' },
      ]);
    });

    const clipItems = page.locator('#results .item:not(.action)');
    await expect(clipItems).toHaveCount(2);
    await expect(clipItems.first().locator('.item-delete')).toBeVisible();

    await page.locator('#input').fill('サンプル');
    await expect(clipItems).toHaveCount(1);
    await expect(clipItems.first()).toContainText('サンプルテキスト');
    await expect(page.locator('#results .item.action')).toBeVisible();
  });

  test('Escape でセレクターが閉じる', async ({ page }) => {
    await expect(page.locator('#container')).toBeVisible();
    const closed = page.waitForEvent('close', { timeout: 15_000 });
    await Promise.all([
      closed,
      page.keyboard.press('Escape').catch(() => undefined),
    ]);
    expect(page.isClosed()).toBe(true);
  });
});

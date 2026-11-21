import { test, expect, TINY_PNG_DATA_URL, expectWebViewCsp } from './fixtures';

test.use({ e2eWindow: 'clip' });

test.describe('登録クリップセレクター WebView', () => {
    test('検索入力と結果領域が表示される', async ({ page }) => {
        await expect(page.locator('#input')).toBeVisible();
        await expect(page.locator('#results')).toBeVisible();
        await expect(page.locator('#container')).toBeVisible();
        await expectWebViewCsp(page);
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
                        items: Array<{ id: string; label: string; preview?: string; kind: string }>
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

    test('× で行内の削除確認に切り替わり、Esc で閉じずに取り消せる', async ({ page }) => {
        await page.evaluate(() => {
            (
                window as unknown as {
                    refreshItems: (
                        items: Array<{ id: string; label: string; preview?: string; kind: string }>
                    ) => void;
                }
            ).refreshItems([
                { id: '0', label: '先頭クリップ', preview: 'first', kind: 'text' },
                { id: '1', label: '末尾クリップ', preview: 'last', kind: 'text' },
            ]);
        });

        const clipItems = page.locator('#results .item:not(.action)');
        await expect(clipItems).toHaveCount(2);
        await clipItems.first().locator('.item-delete').click();
        await expect(clipItems.first().locator('.delete-confirm-ok')).toHaveText('削除する');
        await expect(clipItems.first().locator('.item-delete')).toHaveCount(0);
        await expect(clipItems.nth(1).locator('.item-delete')).toBeVisible();

        await page.keyboard.press('Escape');
        await expect(page.locator('#container')).toBeVisible();
        await expect(clipItems.first().locator('.delete-confirm')).toHaveCount(0);
        await expect(clipItems.first().locator('.item-delete')).toBeVisible();

        await clipItems.first().locator('.item-delete').click();
        await clipItems.first().locator('.delete-confirm-cancel').click();
        await expect(clipItems.first().locator('.item-delete')).toBeVisible();
    });

    test('矢印キーで選択行が移動し Enter で閉じる', async ({ page }) => {
        await page.evaluate(() => {
            (
                window as unknown as {
                    refreshItems: (
                        items: Array<{ id: string; label: string; preview?: string; kind: string }>
                    ) => void;
                }
            ).refreshItems([
                { id: '0', label: '先頭クリップ', preview: 'first', kind: 'text' },
                { id: '1', label: '次のクリップ', preview: 'second', kind: 'text' },
            ]);
        });

        const items = page.locator('#results .item:not(.action)');
        await expect(items).toHaveCount(2);
        await page.locator('#input').click();
        // refreshItems 直後は登録行が選択されていることがある
        await page.keyboard.press('Control+Home');
        await expect(items.nth(0)).toHaveClass(/selected/);

        await page.keyboard.press('ArrowDown');
        await expect(items.nth(1)).toHaveClass(/selected/);

        const closed = page.waitForEvent('close', { timeout: 15_000 });
        await Promise.all([closed, page.keyboard.press('Enter').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);
    });

    test('並べ替えボタンは端で無効になり、検索中は表示しない', async ({ page }) => {
        await page.evaluate(() => {
            (
                window as unknown as {
                    refreshItems: (
                        items: Array<{ id: string; label: string; preview?: string; kind: string }>
                    ) => void;
                }
            ).refreshItems([
                { id: '0', label: '先頭クリップ', preview: 'first', kind: 'text' },
                { id: '1', label: '末尾クリップ', preview: 'last', kind: 'text' },
            ]);
        });

        const clipItems = page.locator('#results .item:not(.action)');
        await expect(clipItems).toHaveCount(2);
        await expect(clipItems.nth(0).locator('.reorder-button[title="上へ移動"]')).toBeDisabled();
        await expect(clipItems.nth(0).locator('.reorder-button[title="下へ移動"]')).toBeEnabled();
        await expect(clipItems.nth(1).locator('.reorder-button[title="上へ移動"]')).toBeEnabled();
        await expect(clipItems.nth(1).locator('.reorder-button[title="下へ移動"]')).toBeDisabled();
        await expect(page.locator('#results .item.action .reorder-buttons')).toHaveCount(0);

        await page.locator('#input').fill('先頭');
        await expect(clipItems).toHaveCount(1);
        await expect(page.locator('#results .reorder-buttons')).toHaveCount(0);
    });

    test('テンプレートのクリップにバッジを表示し、Ctrl+T でも閉じない', async ({ page }) => {
        await page.evaluate(() => {
            (
                window as unknown as {
                    refreshItems: (
                        items: Array<{
                            id: string;
                            label: string;
                            preview?: string;
                            kind: string;
                            template?: boolean;
                        }>
                    ) => void;
                }
            ).refreshItems([
                { id: '0', label: '日報', preview: '{date} 日報', kind: 'text', template: true },
                { id: '1', label: '定型文', preview: 'plain', kind: 'text' },
            ]);
        });

        const clipItems = page.locator('#results .item:not(.action)');
        await expect(clipItems).toHaveCount(2);
        await expect(clipItems.nth(0).locator('.template-badge')).toHaveText('テンプレート');
        await expect(clipItems.nth(1).locator('.template-badge')).toHaveCount(0);

        await page.locator('#input').click();
        await page.keyboard.press('Control+Home');
        await page.keyboard.press('Control+t');
        await expect(page.locator('#container')).toBeVisible();

        await page.locator('#input').fill('テンプレート');
        await expect(clipItems).toHaveCount(1);
        await expect(clipItems.first()).toContainText('日報');
    });

    test('画像クリップのサムネイルとホバープレビューが表示される', async ({ page }) => {
        await page.evaluate((png) => {
            (
                window as unknown as {
                    refreshItems: (
                        items: Array<{
                            id: string;
                            label: string;
                            kind: string;
                            thumbnail?: string;
                            hover_preview?: string;
                        }>
                    ) => void;
                }
            ).refreshItems([
                {
                    id: '0',
                    label: '画像クリップ',
                    kind: 'image',
                    thumbnail: png,
                    hover_preview: png,
                },
            ]);
        }, TINY_PNG_DATA_URL);

        const thumb = page.locator('#results .item--image .item-thumbnail').first();
        await expect(thumb).toBeVisible();
        await expect(thumb).toHaveAttribute('src', TINY_PNG_DATA_URL);

        await thumb.hover();
        const preview = page.locator('#image-hover-preview');
        await expect(preview).toHaveClass(/is-visible/);
        await expect(preview.locator('img')).toHaveAttribute('src', TINY_PNG_DATA_URL);
        await expect(preview.locator('.image-hover-preview-caption')).toContainText('画像クリップ');
    });

    test('Escape でセレクターが閉じる', async ({ page }) => {
        await expect(page.locator('#container')).toBeVisible();
        const closed = page.waitForEvent('close', { timeout: 15_000 });
        // E2E では閉じた後にプロセス終了するため CDP が切れる
        await Promise.all([closed, page.keyboard.press('Escape').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);
    });
});

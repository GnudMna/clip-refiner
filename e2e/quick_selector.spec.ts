import { test, expect, expectWebViewCsp, parseFavoriteModes, readConfigToml } from './fixtures';

test.use({ e2eWindow: 'quick' });

test.describe('クイックセレクター WebView', () => {
    test('検索入力と結果一覧が表示される', async ({ page }) => {
        await expect(page.locator('#input')).toBeVisible();
        await expect(page.locator('#results')).toBeVisible();
        await expect(page.locator('#results .item').first()).toBeVisible();
        await expectWebViewCsp(page);
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
        await Promise.all([closed, page.keyboard.press('Enter').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);
    });

    test('Ctrl+Enter で自動変換に設定して閉じる', async ({ page }) => {
        await expect(page.locator('#results .item').first()).toBeVisible();
        await page.locator('#input').click();
        const closed = page.waitForEvent('close', { timeout: 15_000 });
        await Promise.all([closed, page.keyboard.press('Control+Enter').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);
    });

    test('カテゴリ見出しを ←→ と Enter で開閉できる', async ({ page }) => {
        const header = page.locator('#results .category-toggle', { hasText: 'URL操作' });
        await expect(header).toBeVisible();
        await page.locator('#input').click();

        // 選択行を見出しへ合わせる (マウス移動では入力欄のフォーカスは外れない)
        await header.hover();
        await expect(header).toHaveClass(/selected/);

        if ((await header.getAttribute('aria-expanded')) === 'true') {
            await page.keyboard.press('ArrowLeft');
        }
        await expect(header).toHaveAttribute('aria-expanded', 'false');
        const section = page.locator('#results .category-section', {
            has: page.locator('.category-toggle', { hasText: 'URL操作' }),
        });
        await expect(section).toHaveCount(1);
        await expect(section.locator('.item:not(.category-toggle)')).toHaveCount(0);

        await page.keyboard.press('ArrowRight');
        await expect(header).toHaveAttribute('aria-expanded', 'true');
        await expect(header).toHaveClass(/selected/);
        expect(await section.locator('.item:not(.category-toggle)').count()).toBeGreaterThan(0);

        // カテゴリ内のモード行で ← を押すと見出しへ戻って閉じる
        await page.keyboard.press('ArrowDown');
        await expect(section.locator('.item.selected:not(.category-toggle)')).toHaveCount(1);
        await page.keyboard.press('ArrowLeft');
        await expect(header).toHaveAttribute('aria-expanded', 'false');
        await expect(header).toHaveClass(/selected/);

        await page.keyboard.press('Enter');
        await expect(header).toHaveAttribute('aria-expanded', 'true');
    });

    test('再表示時にアプリ側で保持した展開状態を復元する', async ({ page }) => {
        const header = page.locator('#results .category-toggle', { hasText: '行の並べ替え・削除' });
        await expect(header).toBeVisible();

        await page.evaluate(() => {
            (
                window as unknown as {
                    focusInput: (
                        modeId: string | null,
                        modes?: unknown,
                        expanded?: string[]
                    ) => void;
                }
            ).focusInput(null, undefined, ['行の並べ替え・削除']);
        });
        await expect(header).toHaveAttribute('aria-expanded', 'true');
        await expect(
            page.locator('#results .category-toggle', { hasText: 'URL操作' })
        ).toHaveAttribute('aria-expanded', 'false');
    });

    test('検索中はカテゴリ見出しを出さずに一致したモードを並べる', async ({ page }) => {
        await expect(page.locator('#results .category-toggle').first()).toBeVisible();
        await page.locator('#input').fill('url');
        await expect(page.locator('#results .category-toggle')).toHaveCount(0);
        await expect(page.locator('#results .item').first()).toContainText(/URL/);
    });

    test('Escape でセレクターが閉じる', async ({ page }) => {
        await expect(page.locator('#container')).toBeVisible();
        const closed = page.waitForEvent('close', { timeout: 15_000 });
        // E2E では閉じた後にプロセス終了するため CDP が切れる
        await Promise.all([closed, page.keyboard.press('Escape').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);
    });
});

test.describe('クイックセレクター 連続記録中', () => {
    test.use({
        e2eWindow: 'quick',
        harnessOptions: { continuousRecordEnabled: true },
    });

    test('Ctrl+Enter で自動変換に設定すると連続記録が無効になる', async ({ page, configDir }) => {
        await expect(page.locator('#results .item').first()).toBeVisible();
        await page.locator('#input').click();
        await page.keyboard.press('Home');
        const closed = page.waitForEvent('close', { timeout: 15_000 });
        await Promise.all([closed, page.keyboard.press('Control+Enter').catch(() => undefined)]);
        expect(page.isClosed()).toBe(true);

        await expect
            .poll(() => readConfigToml(configDir), { timeout: 10_000 })
            .toMatch(/^auto_refine_enabled\s*=\s*true$/m);
        expect(readConfigToml(configDir)).toMatch(/^continuous_record_enabled\s*=\s*false$/m);
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

    test('Ctrl+Shift+↓ でお気に入りの順序が config.toml に保存される', async ({
        page,
        configDir,
    }) => {
        const first = page.locator('#results .item').first();
        await expect(first).toContainText('URLデコード');
        await page.locator('#input').click();
        await page.keyboard.press('Home');
        await page.keyboard.press('Control+Shift+ArrowDown');

        await expect
            .poll(() => parseFavoriteModes(readConfigToml(configDir)), { timeout: 10_000 })
            .toEqual(['Trim', 'UrlDecode']);
    });
});

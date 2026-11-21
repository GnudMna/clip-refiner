import { test, expect, parseFavoriteModes, readConfigToml, writeConfigToml, expectWebViewCsp } from './fixtures';

test.use({ e2eWindow: 'settings' });

test.describe('設定 WebView', () => {
  test('8 タブのサイドバーが表示される', async ({ page }) => {
    await expect(page.locator('#sidebar')).toHaveAttribute('role', 'tablist');
    await expect(page.locator('#sidebar')).toHaveAttribute('aria-orientation', 'vertical');
    await expect(page.locator('#sidebar [role="tab"]')).toHaveCount(8);
    await expect(page.locator('#sidebar [role="tab"][data-tab="general"]')).toBeVisible();
    await expect(page.locator('#sidebar [role="tab"][data-tab="shortcuts"]')).toBeVisible();
    await expect(page.locator('#sidebar [role="tab"][aria-selected="true"]')).toHaveCount(1);
    await expect(page.locator('#header h1')).toHaveText('ClipRefiner');
    await expectWebViewCsp(page);
  });

  test('矢印キーで設定タブを移動できる', async ({ page }) => {
    const general = page.locator('#sidebar [role="tab"][data-tab="general"]');
    await general.focus();
    await page.keyboard.press('ArrowDown');
    await expect(page.locator('#sidebar [role="tab"][data-tab="favorites"]')).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await expect(page.locator('#panel-favorites')).toBeVisible();
    await expect(page.locator('#panel-favorites')).toHaveAttribute('role', 'tabpanel');
    await page.keyboard.press('End');
    await expect(page.locator('#sidebar [role="tab"][data-tab="shortcuts"]')).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await expect(page.locator('#panel-shortcuts')).toBeVisible();
  });

  test('ヘッダーに config.toml を開くボタンがある', async ({ page }) => {
    await expect(page.locator('#open-config-btn')).toBeVisible();
    await expect(page.locator('#open-config-btn')).toContainText(/config\.toml/);
  });

  test('監視タブで周期入力ができる', async ({ page }) => {
    await page.locator('#sidebar .nav-btn[data-tab="monitor"]').click();
    const interval = page.locator('#interval-ms');
    await expect(interval).toBeVisible();
    await interval.fill('750');
    await expect(interval).toHaveValue('750');
  });

  test('監視周期の変更が debounce 後に config.toml へ保存される', async ({ page, configDir }) => {
    await page.locator('#sidebar .nav-btn[data-tab="monitor"]').click();
    const interval = page.locator('#interval-ms');
    await interval.fill('750');
    await interval.blur();

    await expect
      .poll(() => /interval_ms\s*=\s*750/.test(readConfigToml(configDir)), {
        timeout: 10_000,
      })
      .toBe(true);
  });

  test('外部で変更された config.toml をバナーで通知する', async ({ page, configDir }) => {
    await expect(page.locator('#app')).toBeVisible();

    const updated = readConfigToml(configDir).replace(
      /interval_ms\s*=\s*\d+/,
      'interval_ms = 2500',
    );
    writeConfigToml(configDir, updated);

    const feedback = page.locator('#apply-feedback');
    await expect(feedback).toBeVisible({ timeout: 15_000 });
    await expect(feedback).toHaveText('設定ファイルが外部で更新されました');

    await page.locator('#sidebar .nav-btn[data-tab="monitor"]').click();
    await expect(page.locator('#interval-ms')).toHaveValue('2500');
  });

  test('正規表現タブでプレビュー領域が表示される', async ({ page }) => {
    await page.locator('#sidebar .nav-btn[data-tab="regex"]').click();
    await expect(page.locator('#regex-preset-name')).toBeVisible();
    await expect(page.locator('#regex-step-pattern-0')).toBeVisible();
    await expect(page.locator('#regex-preview-result')).toBeVisible();
  });

  test('履歴クリアで確認ダイアログが開く', async ({ page }) => {
    await page.locator('#sidebar .nav-btn[data-tab="history"]').click();
    await page.locator('#clear-history-btn').click();
    await expect(page.locator('#confirm-dialog')).toBeVisible();
    await page.locator('#confirm-dialog-cancel').click();
    await expect(page.locator('#confirm-dialog')).toBeHidden();
  });

  test('ショートカットタブに入力欄がある', async ({ page }) => {
    await page.locator('#sidebar .nav-btn[data-tab="shortcuts"]').click();
    await expect(page.locator('#hotkey-quick-selector')).toBeVisible();
  });

  test('お気に入りの並べ替えが config.toml に保存される', async ({ page, configDir }) => {
    await page.locator('#sidebar .nav-btn[data-tab="favorites"]').click();
    await expect(page.locator('.favorite-up[data-mode="UrlDecode"]')).toBeVisible();

    expect(parseFavoriteModes(readConfigToml(configDir))).toEqual(['Trim', 'UrlDecode']);

    await page.locator('.favorite-up[data-mode="UrlDecode"]').click();

    await expect
      .poll(() => parseFavoriteModes(readConfigToml(configDir)), { timeout: 10_000 })
      .toEqual(['UrlDecode', 'Trim']);
  });

  test('閉じるボタンで設定ウィンドウが閉じる', async ({ page }) => {
    await expect(page.locator('#app')).toBeVisible();
    await page.locator('#close-btn').click();
    await expect.poll(() => page.isClosed(), { timeout: 15_000 }).toBe(true);
  });
});

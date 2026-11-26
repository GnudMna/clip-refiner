import {
    test,
    expect,
    parseFavoriteModes,
    readConfigToml,
    writeConfigToml,
    expectWebViewCsp,
} from '../support/fixtures.ts';

test.use({ e2eWindow: 'settings' });

test.describe('設定 WebView', () => {
    test('8 タブのサイドバーが表示される', async ({ page }) => {
        await expect(page.locator('#sidebar')).toHaveAttribute('role', 'tablist');
        await expect(page.locator('#sidebar')).toHaveAttribute('aria-orientation', 'vertical');
        await expect(page.locator('#sidebar [role="tab"]')).toHaveCount(8);
        await expect(page.locator('#sidebar [role="tab"][data-tab="monitor"]')).toBeVisible();
        await expect(page.locator('#sidebar [role="tab"][data-tab="convert"]')).toBeVisible();
        await expect(page.locator('#sidebar [role="tab"][data-tab="general"]')).toBeVisible();
        await expect(page.locator('#sidebar [role="tab"][aria-selected="true"]')).toHaveCount(1);
        await expect(page.locator('#sidebar [role="tab"]').first()).toHaveAttribute(
            'data-tab',
            'general'
        );
        await expect(page.locator('#sidebar [role="tab"][data-tab="general"]')).toHaveAttribute(
            'aria-selected',
            'true'
        );
        await expect(page.locator('#header')).toHaveCount(0);
        await expectWebViewCsp(page);
    });

    test('矢印キーで設定タブを移動できる', async ({ page }) => {
        const convert = page.locator('#sidebar [role="tab"][data-tab="convert"]');
        await convert.focus();
        await page.keyboard.press('ArrowDown');
        await expect(page.locator('#sidebar [role="tab"][data-tab="regex"]')).toHaveAttribute(
            'aria-selected',
            'true'
        );
        await expect(page.locator('#panel-regex')).toBeVisible();
        await expect(page.locator('#panel-regex')).toHaveAttribute('role', 'tabpanel');
        await page.keyboard.press('Home');
        await expect(page.locator('#sidebar [role="tab"][data-tab="general"]')).toHaveAttribute(
            'aria-selected',
            'true'
        );
        await expect(page.locator('#panel-general')).toBeVisible();
    });

    test('サイドバー下部に config.toml を開くボタンがある', async ({ page }) => {
        await expect(page.locator('#side-actions #open-config-btn')).toBeVisible();
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
            'interval_ms = 2500'
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
        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        await page.locator('#clear-history-btn').click();
        await expect(page.locator('#confirm-dialog')).toBeVisible();
        await page.locator('#confirm-dialog-cancel').click();
        await expect(page.locator('#confirm-dialog')).toBeHidden();
    });

    test('登録クリップの編集ダイアログで検証・Esc 閉じ・失敗表示ができる', async ({ page }) => {
        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        // 実ストアは空のため、一覧は画面側にだけ差し込む。画像行には編集ボタンが出ない
        const injectClips = () =>
            page.evaluate(() => {
                window.updateClips([
                    {
                        id: '0',
                        label: 'メモ',
                        preview: 'hello',
                        kind: 'text',
                        token: '0000000000000001',
                    },
                    {
                        id: '1',
                        label: '画像',
                        preview: '',
                        kind: 'image',
                        token: '0000000000000002',
                    },
                ]);
            });
        await injectClips();
        await expect(page.locator('.clip-edit')).toHaveCount(1);

        // 存在しないクリップの編集要求は Rust が拒否し、バナーで伝える。一覧は実ストアの内容へ戻る
        await page.locator('.clip-edit').click();
        await expect(page.locator('#apply-feedback')).toHaveText(
            '登録クリップの一覧が更新されたため、操作を中止しました。もう一度操作してください'
        );
        await expect(page.locator('#clip-editor')).toBeHidden();
        await expect(page.locator('.clip-edit')).toHaveCount(0);

        await injectClips();
        await page.evaluate(() => {
            window.showClipEditor({
                index: 0,
                token: '0000000000000001',
                label: 'メモ',
                text: 'hello\nworld',
            });
        });
        await expect(page.locator('#clip-editor')).toBeVisible();
        await expect(page.locator('#clip-editor-label')).toHaveValue('メモ');
        await expect(page.locator('#clip-editor-text')).toHaveValue('hello\nworld');
        await expect(page.locator('#clip-editor-text')).toBeFocused();

        // 本文が空白のみなら保存せず、ダイアログ内にエラーを出す
        await page.locator('#clip-editor-text').fill('   ');
        await page.locator('#clip-editor-save').click();
        await expect(page.locator('#clip-editor-error')).toHaveText('本文を入力してください');
        await expect(page.locator('#clip-editor')).toBeVisible();

        // 保存の失敗 (対象が実ストアに無い) はダイアログを閉じずに理由を表示する
        await page.locator('#clip-editor-text').fill('changed');
        await page.locator('#clip-editor-save').click();
        await expect(page.locator('#clip-editor-error')).toHaveText(
            '登録クリップが見つかりません。一覧を開き直してください'
        );
        await expect(page.locator('#clip-editor-save')).toBeEnabled();

        // Esc はダイアログだけを閉じ、設定ウィンドウは閉じない。入力した本文も残さない
        await page.keyboard.press('Escape');
        await expect(page.locator('#clip-editor')).toBeHidden();
        await expect(page.locator('#clip-editor-text')).toHaveValue('');
        await expect(page.locator('#app')).toBeVisible();
    });

    test('登録クリップ一覧の操作ボタンは最小幅でも見切れない', async ({ page }) => {
        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        const longPreview = 'とても長いプレビュー '.repeat(20);
        await page.evaluate((preview) => {
            window.updateClips([
                { id: '0', label: '請求書', preview, kind: 'text', template: true, counter: 12 },
                { id: '1', label: 'メモ', preview, kind: 'text' },
                { id: '2', label: '画像', preview: '', kind: 'image' },
            ]);
        }, longPreview);
        await expect(page.locator('.clip-counter-reset')).toHaveCount(1);

        // 設定ウィンドウの既定幅 (960) と最小幅 (640) で、全ボタンが一覧の枠内に収まる
        for (const width of [960, 640]) {
            await page.setViewportSize({ width, height: 640 });
            const overflow = await page.evaluate(() => {
                const wrap = document.querySelector('.list-table-wrap') as HTMLElement;
                const frame = wrap.getBoundingClientRect();
                return Array.from(wrap.querySelectorAll('button'))
                    .map((button) => button.getBoundingClientRect())
                    .filter(
                        (rect) => rect.left < frame.left - 0.5 || rect.right > frame.right + 0.5
                    ).length;
            });
            expect(overflow, `幅 ${width}px で枠からはみ出したボタン数`).toBe(0);
        }
    });

    test('編集中に一覧が変わったらダイアログを閉じる', async ({ page }) => {
        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        const clips = [
            { id: '0', label: 'a', preview: 'pa', kind: 'text', token: '0000000000000001' },
            { id: '1', label: 'b', preview: 'pb', kind: 'text', token: '0000000000000002' },
        ];
        await page.evaluate((list) => {
            window.updateClips(list);
            window.showClipEditor({ index: 1, token: '0000000000000002', label: 'b', text: 'pb' });
        }, clips);
        await expect(page.locator('#clip-editor')).toBeVisible();

        // 同じ内容の再送では閉じない
        await page.evaluate((list) => window.updateClips(list), clips);
        await expect(page.locator('#clip-editor')).toBeVisible();

        // 先頭が削除されて別のクリップがそのインデックスへ来たら閉じる
        await page.evaluate((list) => window.updateClips(list.slice(0, 1)), clips);
        await expect(page.locator('#clip-editor')).toBeHidden();
        await expect(page.locator('#apply-feedback')).toContainText('編集を閉じました');
    });

    test('ショートカットタブに入力欄がある', async ({ page }) => {
        await page.locator('#sidebar .nav-btn[data-tab="shortcuts"]').click();
        await expect(page.locator('#hotkey-quick-selector')).toBeVisible();
    });

    test('情報タブに利用者向けドキュメントと THIRD_PARTY_NOTICES を開くボタンがある', async ({
        page,
    }) => {
        await page.locator('#sidebar .nav-btn[data-tab="about"]').click();
        await expect(page.locator('#panel-about')).toBeVisible();
        await expect(page.locator('#open-user-docs-btn')).toBeVisible();
        await expect(page.locator('#open-third-party-notices-btn')).toBeVisible();
    });

    test('情報タブにこのアプリケーションのライセンスが表示される', async ({ page }) => {
        await page.locator('#sidebar .nav-btn[data-tab="about"]').click();
        await expect(page.locator('#panel-about')).toContainText('Copyright (c) 2026 Yuto Hirata');
        await expect(page.locator('#panel-about')).toContainText('MIT License');
        await expect(page.locator('#open-license-btn')).toBeVisible();
    });

    test('お気に入りの並べ替えが config.toml に保存される', async ({ page, configDir }) => {
        await page.locator('#sidebar .nav-btn[data-tab="convert"]').click();
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

    test('連続記録を有効にすると自動変換のチェックが外れる', async ({ page, configDir }) => {
        // v3 からの移行で自動変換は有効になっている
        await page.locator('#sidebar .nav-btn[data-tab="convert"]').click();
        await expect(page.locator('#auto-refine-enabled')).toBeChecked();

        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        await page.locator('#record-enabled').check();

        await expect
            .poll(() => readConfigToml(configDir), { timeout: 10_000 })
            .toMatch(/^continuous_record_enabled\s*=\s*true$/m);
        expect(readConfigToml(configDir)).toMatch(/^auto_refine_enabled\s*=\s*false$/m);

        await page.locator('#sidebar .nav-btn[data-tab="convert"]').click();
        await expect(page.locator('#auto-refine-enabled')).not.toBeChecked({ timeout: 10_000 });
    });
});

test.describe('設定 WebView 連続記録中', () => {
    test.use({ harnessOptions: { continuousRecordEnabled: true } });

    test('自動変換を有効にすると連続記録のチェックが外れる', async ({ page, configDir }) => {
        await page.locator('#sidebar .nav-btn[data-tab="convert"]').click();
        await expect(page.locator('#monitor-record-banner')).toBeVisible();
        await expect(page.locator('#auto-refine-enabled')).not.toBeChecked();

        await page.locator('#auto-refine-enabled').check();

        await expect
            .poll(() => readConfigToml(configDir), { timeout: 10_000 })
            .toMatch(/^auto_refine_enabled\s*=\s*true$/m);
        expect(readConfigToml(configDir)).toMatch(/^continuous_record_enabled\s*=\s*false$/m);
        await expect(page.locator('#monitor-record-banner')).toBeHidden({ timeout: 10_000 });

        await page.locator('#sidebar .nav-btn[data-tab="clipboard"]').click();
        await expect(page.locator('#record-enabled')).not.toBeChecked();
    });
});

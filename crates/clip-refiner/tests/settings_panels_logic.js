/**
 * `SettingsPanelsLogic` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';

import SettingsPanelsLogic from '../src/ui/settings/panels_logic.js';
import SettingsSharedLogic from '../src/ui/settings/shared_logic.js';

/** パイプライン行を上下移動・削除できること */
function test_apply_pipeline_action() {
    assert.deepEqual(SettingsPanelsLogic.applyPipelineAction(['a', 'b', 'c'], 1, 'up'), [
        'b',
        'a',
        'c',
    ]);
    assert.deepEqual(SettingsPanelsLogic.applyPipelineAction(['a', 'b', 'c'], 1, 'down'), [
        'a',
        'c',
        'b',
    ]);
    assert.deepEqual(SettingsPanelsLogic.applyPipelineAction(['a', 'b', 'c'], 1, 'remove'), [
        'a',
        'c',
    ]);
    assert.equal(SettingsPanelsLogic.applyPipelineAction(['a', 'b'], 0, 'up'), null);
    assert.equal(SettingsPanelsLogic.applyPipelineAction(['a', 'b'], 1, 'down'), null);
    assert.equal(SettingsPanelsLogic.applyPipelineAction(['a'], 3, 'remove'), null);
}

/** パイプラインへモードを追加できること */
function test_append_pipeline_mode() {
    assert.deepEqual(SettingsPanelsLogic.appendPipelineMode(['a'], 'b', 3), ['a', 'b']);
    assert.equal(SettingsPanelsLogic.appendPipelineMode(['a', 'b'], 'c', 2), null);
    assert.equal(SettingsPanelsLogic.appendPipelineMode(['a'], '', 3), null);
}

/** サイドバー tablist HTML を組み立てられること */
function test_build_sidebar_nav_inner_html() {
    const html = SettingsPanelsLogic.buildSidebarNavInnerHtml(
        [
            { id: 'general', label: '一般' },
            { id: 'monitor', label: '監視' },
        ],
        'monitor',
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(html.includes('role="tab"'));
    assert.ok(html.includes('id="tab-monitor"'));
    assert.ok(html.includes('data-tab="monitor"'));
    assert.ok(html.includes('aria-controls="panel-monitor"'));
    assert.ok(html.includes('aria-selected="true"'));
    assert.ok(html.includes('tabindex="0"'));
    assert.ok(html.includes('class="nav-btn active"'));
    assert.ok(html.includes('一般'));
    assert.ok(html.includes('id="tab-general"'));
    assert.ok(html.includes('aria-selected="false"'));
    assert.ok(html.includes('tabindex="-1"'));
    assert.ok(!html.includes('aria-current'));
}

/** タブ選択状態の aria / tabindex を切り替えられること */
function test_tab_selection_state() {
    assert.deepEqual(SettingsPanelsLogic.tabSelectionState(true), {
        ariaSelected: 'true',
        tabIndex: '0',
    });
    assert.deepEqual(SettingsPanelsLogic.tabSelectionState(false), {
        ariaSelected: 'false',
        tabIndex: '-1',
    });
}

/** 縦方向 tablist のキーボード移動先を計算できること */
function test_next_tab_index_for_key() {
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(0, 3, 'ArrowDown'), 1);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(2, 3, 'ArrowDown'), 0);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(0, 3, 'ArrowRight'), 1);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(0, 3, 'ArrowUp'), 2);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(1, 3, 'ArrowLeft'), 0);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(2, 3, 'Home'), 0);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(0, 3, 'End'), 2);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(1, 3, 'Enter'), null);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(-1, 3, 'ArrowDown'), null);
    assert.equal(SettingsPanelsLogic.nextTabIndexForKey(0, 0, 'ArrowDown'), null);
}

/** 登録クリップ行 HTML を組み立てられること */
function test_render_clip_row() {
    const textHtml = SettingsPanelsLogic.renderClipRow(
        { id: '0', label: 'メモ', preview: 'hello', kind: 'text' },
        SettingsSharedLogic.escapeHtml,
        0,
        2
    );
    assert.ok(textHtml.includes('clip-kind--text'));
    assert.ok(textHtml.includes('data-index="0"'));
    // テンプレート切り替えは画像行と位置を揃えるため操作列の先頭に置く
    assert.ok(textHtml.indexOf('clip-template') < textHtml.indexOf('clip-up'));
    // テキストはテンプレート切り替えボタンを持ち、無効時は有効化を要求する
    assert.ok(textHtml.includes('clip-template'));
    assert.ok(textHtml.includes('data-enabled="true"'));
    assert.ok(textHtml.includes('aria-pressed="false"'));
    assert.ok(!textHtml.includes('clip-thumbnail'));
    // 先頭行は上へ移動できず、下へは移動できる
    assert.ok(textHtml.includes('title="上へ" disabled'));
    assert.ok(!textHtml.includes('title="下へ" disabled'));

    const imageHtml = SettingsPanelsLogic.renderClipRow(
        {
            id: '1',
            label: '画像',
            preview: 'preview',
            kind: 'image',
            thumbnail: 'data:image/png;base64,abc',
        },
        SettingsSharedLogic.escapeHtml,
        1,
        2
    );
    assert.ok(imageHtml.includes('clip-kind--image'));
    assert.ok(!imageHtml.includes('clip-template'));

    const templateHtml = SettingsPanelsLogic.renderClipRow(
        { id: '2', label: '日報', preview: '{date}', kind: 'text', template: true },
        SettingsSharedLogic.escapeHtml,
        1,
        3
    );
    // 有効時はボタンを強調し、押下で無効化を要求する
    assert.ok(templateHtml.includes('clip-template is-active'));
    assert.ok(templateHtml.includes('data-enabled="false"'));
    assert.ok(templateHtml.includes('aria-pressed="true"'));
    assert.ok(!imageHtml.includes('title="上へ" disabled'));
    assert.ok(imageHtml.includes('title="下へ" disabled'));
    assert.ok(imageHtml.includes('clip-thumbnail'));
}

/** お気に入り行 HTML を組み立てられること */
function test_build_favorite_rows_html() {
    const html = SettingsPanelsLogic.buildFavoriteRowsHtml(
        ['Trim'],
        (modeId) => (modeId === 'Trim' ? '前後空白除去' : modeId),
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(html.includes('前後空白除去'));
    assert.ok(html.includes('favorite-up'));
    assert.ok(html.includes('data-mode="Trim"'));
}

/** お気に入り未登録の追加候補を抽出できること */
function test_available_favorite_modes() {
    const available = SettingsPanelsLogic.availableFavoriteModes(
        [
            { id: 'Trim', label: 'Trim' },
            { id: 'UrlDecode', label: 'URL' },
        ],
        ['Trim']
    );
    assert.deepEqual(available, [{ id: 'UrlDecode', label: 'URL' }]);
}

/** お気に入りスロットの表示値を解決できること */
function test_favorite_slot_display() {
    assert.deepEqual(SettingsPanelsLogic.favoriteSlotDisplay(undefined, 0), {
        value: 'Alt+Shift+1',
        placeholder: 'Alt+Shift+1',
    });
    assert.deepEqual(SettingsPanelsLogic.favoriteSlotDisplay(['Ctrl+1'], 0), {
        value: 'Ctrl+1',
        placeholder: 'Alt+Shift+1',
    });
    assert.deepEqual(SettingsPanelsLogic.favoriteSlotDisplay([], 9), {
        value: 'Alt+Shift+F1',
        placeholder: 'Alt+Shift+F1',
    });
}

/** コンテンツクリック委譲アクションを解決できること */
function test_resolve_content_click_action() {
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'pipeline-up';
                },
            },
            dataset: { index: '2' },
        }),
        { type: 'pipeline', action: 'up', index: 2 }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'favorite-down';
                },
            },
            dataset: { mode: 'Trim' },
        }),
        { type: 'favorite-move', mode: 'Trim', direction: 'down' }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-copy';
                },
            },
            dataset: { index: '3' },
        }),
        { type: 'clip-copy', index: 3 }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-up';
                },
            },
            dataset: { index: '1' },
        }),
        { type: 'clip-move', index: 1, direction: 'up' }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-template';
                },
            },
            dataset: { index: '2', enabled: 'true' },
        }),
        { type: 'clip-template', index: 2, enabled: true }
    );
    assert.equal(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains() {
                    return false;
                },
            },
            dataset: {},
        }),
        null
    );
}

/** 登録クリップボタンの表示状態を切り替えられること */
function test_clip_register_button_state() {
    assert.deepEqual(SettingsPanelsLogic.clipRegisterButtonState(false), {
        className: 'ghost-btn',
        label: 'クリップボードから登録',
        disabled: false,
    });
    assert.deepEqual(SettingsPanelsLogic.clipRegisterButtonState(true), {
        className: 'ghost-btn is-loading',
        label: '登録中...',
        disabled: true,
    });
}

/** バイト数表示を単位付きで整形できること */
function test_format_bytes() {
    assert.equal(SettingsPanelsLogic.formatBytes(512), '512 B');
    assert.equal(SettingsPanelsLogic.formatBytes(2048), '2.0 KB');
    assert.equal(SettingsPanelsLogic.formatBytes(2 * 1024 * 1024), '2.0 MB');
    assert.equal(SettingsPanelsLogic.formatBytes(-1), '');
    assert.equal(SettingsPanelsLogic.formatBytes(undefined), '');
}

/** 更新ステータス HTML を状態ごとに組み立てられること */
function test_render_update_status_html() {
    const escapeHtml = (text) =>
        String(text)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');

    const empty = SettingsPanelsLogic.renderUpdateStatusHtml(null, false, escapeHtml);
    assert.ok(empty.includes('更新を確認'));

    const upToDate = SettingsPanelsLogic.renderUpdateStatusHtml(
        { status: 'up-to-date', latestVersion: '0.13.1' },
        false,
        escapeHtml
    );
    assert.ok(upToDate.includes('最新版です (v0.13.1)'));

    const available = SettingsPanelsLogic.renderUpdateStatusHtml(
        {
            status: 'available',
            latestVersion: '0.14.0',
            size: 2048,
        },
        false,
        escapeHtml
    );
    assert.ok(available.includes('v0.14.0 が利用可能です'));
    assert.ok(available.includes('2.0 KB'));
    assert.ok(available.includes('apply-update-btn'));
    assert.ok(!available.includes(' disabled'));

    const busyAvailable = SettingsPanelsLogic.renderUpdateStatusHtml(
        {
            status: 'available',
            latestVersion: '0.14.0',
            size: 100,
        },
        true,
        escapeHtml
    );
    assert.ok(busyAvailable.includes('適用中...'));
    assert.ok(busyAvailable.includes(' disabled'));

    const errorHtml = SettingsPanelsLogic.renderUpdateStatusHtml(
        { status: 'error', message: '失敗 <msg>' },
        false,
        escapeHtml
    );
    assert.ok(errorHtml.includes('失敗 &lt;msg&gt;'));
    assert.ok(!errorHtml.includes('失敗 <msg>'));
}

const tests = [
    test_apply_pipeline_action,
    test_append_pipeline_mode,
    test_build_sidebar_nav_inner_html,
    test_tab_selection_state,
    test_next_tab_index_for_key,
    test_render_clip_row,
    test_build_favorite_rows_html,
    test_available_favorite_modes,
    test_favorite_slot_display,
    test_resolve_content_click_action,
    test_clip_register_button_state,
    test_format_bytes,
    test_render_update_status_html,
];

for (const test of tests) {
    test();
}

console.log(`settings_panels_logic: ${tests.length} passed`);

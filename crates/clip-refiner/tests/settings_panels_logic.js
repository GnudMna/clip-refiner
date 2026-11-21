/**
 * `SettingsPanelsLogic` 純粋ロジックの Node テスト
 */
'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');

const SettingsPanelsLogic = require(path.join(__dirname, '../src/ui/settings/panels_logic.js'));
const SettingsSharedLogic = require(path.join(__dirname, '../src/ui/settings/shared_logic.js'));

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

/** サイドバーナビ HTML を組み立てられること */
function test_build_sidebar_nav_inner_html() {
    const html = SettingsPanelsLogic.buildSidebarNavInnerHtml(
        [
            { id: 'general', label: '一般' },
            { id: 'monitor', label: '監視' },
        ],
        'monitor',
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(html.includes('data-tab="monitor"'));
    assert.ok(html.includes('aria-current="page"'));
    assert.ok(html.includes('class="nav-btn active"'));
    assert.ok(html.includes('一般'));
}

/** 登録クリップ行 HTML を組み立てられること */
function test_render_clip_row() {
    const textHtml = SettingsPanelsLogic.renderClipRow(
        { id: '0', label: 'メモ', preview: 'hello', kind: 'text' },
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(textHtml.includes('clip-kind--text'));
    assert.ok(textHtml.includes('data-index="0"'));
    assert.ok(!textHtml.includes('clip-thumbnail'));

    const imageHtml = SettingsPanelsLogic.renderClipRow(
        {
            id: '1',
            label: '画像',
            preview: 'preview',
            kind: 'image',
            thumbnail: 'data:image/png;base64,abc',
        },
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(imageHtml.includes('clip-kind--image'));
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

const tests = [
    test_apply_pipeline_action,
    test_append_pipeline_mode,
    test_build_sidebar_nav_inner_html,
    test_render_clip_row,
    test_build_favorite_rows_html,
    test_available_favorite_modes,
    test_favorite_slot_display,
    test_resolve_content_click_action,
    test_clip_register_button_state,
];

for (const test of tests) {
    test();
}

console.log(`settings_panels_logic: ${tests.length} passed`);

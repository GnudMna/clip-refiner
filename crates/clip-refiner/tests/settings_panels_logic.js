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

    // 連番は払い出し済みのテキストだけに表示し、リセットボタンは他のボタンより前に置く
    assert.ok(!textHtml.includes('clip-counter'));
    assert.ok(!templateHtml.includes('clip-counter'));
    const counterHtml = SettingsPanelsLogic.renderClipRow(
        {
            id: '3',
            label: '請求書',
            preview: 'No.{counter}',
            kind: 'text',
            template: true,
            counter: 5,
        },
        SettingsSharedLogic.escapeHtml,
        0,
        3
    );
    assert.ok(counterHtml.includes('連番 5'));
    assert.ok(counterHtml.includes('clip-counter-reset'));
    assert.ok(counterHtml.indexOf('clip-counter-reset') < counterHtml.indexOf('clip-template'));
    const imageCounterHtml = SettingsPanelsLogic.renderClipRow(
        { id: '4', label: '画像', kind: 'image', counter: 2 },
        SettingsSharedLogic.escapeHtml,
        0,
        3
    );
    assert.ok(!imageCounterHtml.includes('clip-counter'));
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
            dataset: { index: '3', token: 'aa' },
        }),
        { type: 'clip-copy', index: 3, token: 'aa' }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-up';
                },
            },
            dataset: { index: '1', token: 'bb' },
        }),
        { type: 'clip-move', index: 1, token: 'bb', direction: 'up' }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-template';
                },
            },
            dataset: { index: '2', token: 'cc', enabled: 'true' },
        }),
        { type: 'clip-template', index: 2, token: 'cc', enabled: true }
    );
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-counter-reset';
                },
            },
            dataset: { index: '4', token: 'dd' },
        }),
        { type: 'clip-counter-reset', index: 4, token: 'dd' }
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

/** 編集ボタンがテキスト行だけにあり、↑↓ より前に置かれること */
function test_render_clip_row_edit_button() {
    const textHtml = SettingsPanelsLogic.renderClipRow(
        { id: '4', label: 'メモ', preview: 'hello', kind: 'text' },
        SettingsSharedLogic.escapeHtml,
        0,
        2
    );
    assert.ok(textHtml.includes('clip-edit" data-index="4"'));
    assert.ok(textHtml.indexOf('clip-edit') < textHtml.indexOf('clip-up'));

    const imageHtml = SettingsPanelsLogic.renderClipRow(
        { id: '5', label: '画像', kind: 'image' },
        SettingsSharedLogic.escapeHtml,
        1,
        2
    );
    assert.ok(!imageHtml.includes('clip-edit'));
}

/** 各操作ボタンが表示時のインデックスと識別トークンを持つこと */
function test_render_clip_row_carries_token() {
    const html = SettingsPanelsLogic.renderClipRow(
        { id: '2', label: 'メモ', preview: 'hello', kind: 'text', counter: 3, token: 'ab"<c' },
        SettingsSharedLogic.escapeHtml,
        1,
        3
    );
    const buttons = html.match(/<button [^>]*data-index="2"[^>]*>/g) || [];
    // 連番リセット・テンプレート・編集・↑・↓・コピー・削除
    assert.equal(buttons.length, 7);
    for (const button of buttons) {
        assert.ok(button.includes('data-token="ab&quot;&lt;c"'), button);
    }

    // トークンが無い一覧でも属性が壊れない
    const noToken = SettingsPanelsLogic.renderClipRow(
        { id: '0', label: 'a', kind: 'text' },
        SettingsSharedLogic.escapeHtml,
        0,
        1
    );
    assert.ok(noToken.includes('class="ghost-btn clip-copy" data-index="0" data-token="">'));
}

/** 編集ボタンのクリックを編集要求へ解決できること */
function test_resolve_content_click_action_clip_edit() {
    assert.deepEqual(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-edit';
                },
            },
            dataset: { index: '6', token: 'ee' },
        }),
        { type: 'clip-edit', index: 6, token: 'ee' }
    );
}

/** 登録クリップの編集内容を保存前に検証できること */
function test_validate_clip_edit() {
    assert.deepEqual(SettingsPanelsLogic.validateClipEdit('', 'body', 64), { ok: true });
    assert.deepEqual(SettingsPanelsLogic.validateClipEdit('  ', ' a ', 64), { ok: true });

    // 本文が空白のみなら保存できない
    const emptyText = SettingsPanelsLogic.validateClipEdit('label', ' \n\t ', 64);
    assert.equal(emptyText.ok, false);
    assert.equal(emptyText.field, 'text');

    // ラベルは前後の空白を除いたコードポイント数で上限を判定する
    assert.equal(
        SettingsPanelsLogic.validateClipEdit(' ' + 'あ'.repeat(64) + ' ', 'b', 64).ok,
        true
    );
    assert.equal(SettingsPanelsLogic.validateClipEdit('😀'.repeat(64), 'b', 64).ok, true);
    const tooLong = SettingsPanelsLogic.validateClipEdit('あ'.repeat(65), 'b', 64);
    assert.equal(tooLong.ok, false);
    assert.equal(tooLong.field, 'label');
    assert.ok(tooLong.message.includes('64'));
}

/** 編集対象が開いた時点のままかを一覧から判定できること */
function test_clip_editor_target_unchanged() {
    const editor = { index: 1, token: 't2', label: 'b', preview: 'pb' };
    const clips = [
        { label: 'a', preview: 'pa', kind: 'text', token: 't1' },
        { label: 'b', preview: 'pb', kind: 'text', token: 't2' },
    ];
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, clips), true);

    // 削除・並べ替えで別のクリップがそのインデックスへ来た場合は変更扱い
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [clips[1], clips[0]]),
        false
    );
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [clips[0]]), false);
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [
            clips[0],
            { label: 'b', preview: 'pb', kind: 'image', token: 't2' },
        ]),
        false
    );

    // ラベル・プレビューが同じでも、識別トークンが違えば別のクリップ扱い
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [
            clips[0],
            { label: 'b', preview: 'pb', kind: 'text', token: 'other' },
        ]),
        false
    );
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(null, clips), false);
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, undefined), false);
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
    test_render_clip_row_edit_button,
    test_render_clip_row_carries_token,
    test_resolve_content_click_action_clip_edit,
    test_validate_clip_edit,
    test_clip_editor_target_unchanged,
    test_format_bytes,
    test_render_update_status_html,
];

for (const test of tests) {
    test();
}

console.log(`settings_panels_logic: ${tests.length} passed`);

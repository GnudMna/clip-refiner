/**
 * `SettingsPanelsLogic` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';
import { test } from 'node:test';

import SettingsPanelsLogic from '../../src/ui/settings/panels_logic.ts';
import SettingsSharedLogic from '../../src/ui/settings/shared_logic.ts';

/**
 * 添字アクセスの結果が存在することを検証して返す (noUncheckedIndexedAccess 対策)
 */
function at<T>(list: readonly T[], index: number): T {
    const value = list[index];
    assert.notEqual(value, undefined, 'index ' + index + ' が存在しない');
    return value as T;
}

/** パイプライン行を上下移動・削除できること */
test('apply_pipeline_action', () => {
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
});

/** パイプラインへモードを追加できること */
test('append_pipeline_mode', () => {
    assert.deepEqual(SettingsPanelsLogic.appendPipelineMode(['a'], 'b', 3), ['a', 'b']);
    assert.equal(SettingsPanelsLogic.appendPipelineMode(['a', 'b'], 'c', 2), null);
    assert.equal(SettingsPanelsLogic.appendPipelineMode(['a'], '', 3), null);
});

/** サイドバー tablist HTML を組み立てられること */
test('build_sidebar_nav_inner_html', () => {
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
});

/** タブ選択状態の aria / tabindex を切り替えられること */
test('tab_selection_state', () => {
    assert.deepEqual(SettingsPanelsLogic.tabSelectionState(true), {
        ariaSelected: 'true',
        tabIndex: '0',
    });
    assert.deepEqual(SettingsPanelsLogic.tabSelectionState(false), {
        ariaSelected: 'false',
        tabIndex: '-1',
    });
});

/** 縦方向 tablist のキーボード移動先を計算できること */
test('next_tab_index_for_key', () => {
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
});

/** 登録クリップ行 HTML を組み立てられること */
test('render_clip_row', () => {
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
});

/** お気に入り行 HTML を組み立てられること */
test('build_favorite_rows_html', () => {
    const html = SettingsPanelsLogic.buildFavoriteRowsHtml(
        ['Trim'],
        (modeId) => (modeId === 'Trim' ? '前後空白除去' : modeId),
        SettingsSharedLogic.escapeHtml
    );
    assert.ok(html.includes('前後空白除去'));
    assert.ok(html.includes('favorite-up'));
    assert.ok(html.includes('data-mode="Trim"'));
});

/** お気に入り未登録の追加候補を抽出できること */
test('available_favorite_modes', () => {
    const available = SettingsPanelsLogic.availableFavoriteModes(
        [
            { id: 'Trim', label: 'Trim' },
            { id: 'UrlDecode', label: 'URL' },
        ],
        ['Trim']
    );
    assert.deepEqual(available, [{ id: 'UrlDecode', label: 'URL' }]);
});

/** お気に入りスロットの表示値を解決できること */
test('favorite_slot_display', () => {
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
});

/** コンテンツクリック委譲アクションを解決できること */
test('resolve_content_click_action', () => {
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
    // 必要な data 属性が欠けたボタンは IPC を送らない
    assert.equal(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'favorite-remove';
                },
            },
            dataset: {},
        }),
        null
    );
    assert.equal(
        SettingsPanelsLogic.resolveContentClickAction({
            classList: {
                contains(name) {
                    return name === 'clip-delete';
                },
            },
            dataset: { index: '0' },
        }),
        null
    );
});

/** 登録クリップボタンの表示状態を切り替えられること */
test('clip_register_button_state', () => {
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
});

/** 編集ボタンがテキスト行だけにあり、↑↓ より前に置かれること */
test('render_clip_row_edit_button', () => {
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
});

/** 各操作ボタンが表示時のインデックスと識別トークンを持つこと */
test('render_clip_row_carries_token', () => {
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
});

/** 編集ボタンのクリックを編集要求へ解決できること */
test('resolve_content_click_action_clip_edit', () => {
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
});

/** 登録クリップの編集内容を保存前に検証できること */
test('validate_clip_edit', () => {
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
});

/** 編集対象が開いた時点のままかを一覧から判定できること */
test('clip_editor_target_unchanged', () => {
    const editor = { index: 1, token: 't2', label: 'b', preview: 'pb' };
    const clips = [
        { label: 'a', preview: 'pa', kind: 'text', token: 't1' },
        { label: 'b', preview: 'pb', kind: 'text', token: 't2' },
    ];
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, clips), true);

    // 削除・並べ替えで別のクリップがそのインデックスへ来た場合は変更扱い
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [at(clips, 1), at(clips, 0)]),
        false
    );
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [at(clips, 0)]), false);
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [
            at(clips, 0),
            { label: 'b', preview: 'pb', kind: 'image', token: 't2' },
        ]),
        false
    );

    // ラベル・プレビューが同じでも、識別トークンが違えば別のクリップ扱い
    assert.equal(
        SettingsPanelsLogic.clipEditorTargetUnchanged(editor, [
            at(clips, 0),
            { label: 'b', preview: 'pb', kind: 'text', token: 'other' },
        ]),
        false
    );
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(null, clips), false);
    assert.equal(SettingsPanelsLogic.clipEditorTargetUnchanged(editor, undefined), false);
});

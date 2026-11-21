/**
 * セレクター共有 / クイック / 登録クリップ純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';

import ClipSelectorLogic from '../src/ui/clip_selector_logic.js';
import QuickSelectorLogic from '../src/ui/quick_selector_logic.js';
import SelectorCommon from '../src/ui/selector-common.js';

/** @type {import('../src/ui/quick_selector_logic').QuickSelectorMode[]} */
const SAMPLE_MODES = [
    {
        id: 'Trim',
        label: '前後空白除去',
        category: 'テキスト',
        value: 'trim',
        favorite: true,
        favoriteIndex: 1,
    },
    {
        id: 'UrlDecode',
        label: 'URL デコード',
        category: 'エンコード',
        value: 'url-decode',
        favorite: true,
        favoriteIndex: 0,
    },
    {
        id: 'JsonFormat',
        label: 'JSON 整形',
        category: 'テキスト',
        value: 'json-format',
        favorite: false,
    },
];

/** @type {import('../src/ui/clip_selector_logic').ClipSelectorItem[]} */
const SAMPLE_CLIPS = [
    { id: '0', label: 'メモ', preview: 'hello world', kind: 'text' },
    {
        id: '1',
        label: 'スクショ',
        preview: '画像プレビュー',
        kind: 'image',
        thumbnail: 'data:image/png;base64,abc',
        hover_preview: 'data:image/png;base64,hover',
    },
];

/** HTML 特殊文字をエスケープできること */
function test_escape_html() {
    assert.equal(SelectorCommon.escapeHtml('<b>&"'), '&lt;b&gt;&amp;&quot;');
}

/** 検索クエリを正規化できること */
function test_normalize_query() {
    assert.equal(SelectorCommon.normalizeQuery('  AbC  '), 'abc');
    assert.equal(SelectorCommon.normalizeQuery(null), '');
}

/** 一致部分をハイライトできること */
function test_highlight_text() {
    assert.equal(SelectorCommon.highlightText('Hello', 'ell'), 'H<mark>ell</mark>o');
    assert.equal(SelectorCommon.highlightText('<b>', 'b'), '&lt;<mark>b</mark>&gt;');
    assert.equal(SelectorCommon.highlightText('Hello', ''), 'Hello');
}

/** 選択インデックスをクランプできること */
function test_clamp_index() {
    assert.equal(SelectorCommon.clampIndex(5, 0), 0);
    assert.equal(SelectorCommon.clampIndex(5, 3), 2);
    assert.equal(SelectorCommon.clampIndex(-1, 3), 0);
    assert.equal(SelectorCommon.clampIndex(1, 3), 1);
}

/** 矢印キーで選択移動を処理できること */
function test_handle_arrow_keys() {
    const moves = [];
    const event = {
        key: 'ArrowDown',
        preventDefault() {
            this.prevented = true;
        },
    };
    assert.equal(
        SelectorCommon.handleArrowKeys(event, (delta) => moves.push(delta)),
        true
    );
    assert.deepEqual(moves, [1]);
    assert.equal(event.prevented, true);
    assert.equal(
        SelectorCommon.handleArrowKeys({ key: 'Enter' }, () => {}),
        false
    );
}

/** Escape でフィルタクリアまたは閉じるを処理できること */
function test_handle_escape_key() {
    let cleared = false;
    let closed = false;
    const input = { value: 'abc' };
    const event = {
        key: 'Escape',
        preventDefault() {
            this.prevented = true;
        },
    };
    assert.equal(
        SelectorCommon.handleEscapeKey(
            event,
            input,
            () => {
                cleared = true;
            },
            () => {
                closed = true;
            }
        ),
        true
    );
    assert.equal(input.value, '');
    assert.equal(cleared, true);
    assert.equal(closed, false);

    assert.equal(
        SelectorCommon.handleEscapeKey(
            event,
            input,
            () => {
                cleared = false;
            },
            () => {
                closed = true;
            }
        ),
        true
    );
    assert.equal(closed, true);
}

/** お気に入り ID を表示順で返すこと */
function test_favorite_ids_sorted() {
    assert.deepEqual(QuickSelectorLogic.favoriteIds(SAMPLE_MODES), ['UrlDecode', 'Trim']);
}

/** 空クエリではお気に入り + 全モードの表示一覧を組み立てること */
function test_build_display_list_without_query() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '');
    assert.equal(display.length, 5);
    assert.equal(display[0].id, 'UrlDecode');
    assert.equal(display[0].listSection, 'favorites');
    assert.equal(display[1].id, 'Trim');
    assert.equal(display[1].listSection, 'favorites');
    assert.equal(display[2].listSection, 'category');
}

/** クエリでモードを絞り込めること */
function test_build_display_list_with_query() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'json');
    assert.equal(display.length, 1);
    assert.equal(display[0].id, 'JsonFormat');
}

/** カテゴリ見出しラベルを解決できること */
function test_section_category() {
    const favorite = { id: 'Trim', label: 'Trim', category: 'テキスト', listSection: 'favorites' };
    assert.equal(QuickSelectorLogic.sectionCategory(favorite, ''), 'お気に入り');
    assert.equal(QuickSelectorLogic.sectionCategory(favorite, 't'), null);
    assert.equal(
        QuickSelectorLogic.sectionCategory(
            { id: 'Trim', label: 'Trim', category: 'テキスト', listSection: 'category' },
            ''
        ),
        'テキスト'
    );
}

/** フィルタ後の選択インデックスを維持できること */
function test_next_index_after_filter() {
    const filtered = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'url');
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, 'UrlDecode'), 0);
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, 'missing'), 0);
}

/** お気に入り並べ替え可否を判定できること */
function test_can_move_selected_favorite() {
    const favorite = {
        id: 'Trim',
        label: 'Trim',
        category: 'テキスト',
        favorite: true,
        listSection: 'favorites',
    };
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('', favorite), true);
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('t', favorite), false);
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('', null), false);
}

/** クイックセレクター IPC メッセージを組み立てられること */
function test_quick_selector_ipc_messages() {
    assert.equal(QuickSelectorLogic.selectMessage('Trim'), 'select:Trim');
    assert.equal(QuickSelectorLogic.toggleFavoriteMessage('Trim'), 'toggle-favorite:Trim');
    assert.equal(QuickSelectorLogic.moveFavoriteMessage('Trim', 'up'), 'move-favorite:Trim:up');
}

/** モード行 HTML を組み立てられること */
function test_build_item_inner_html() {
    const html = QuickSelectorLogic.buildItemInnerHtml(
        SAMPLE_MODES[0],
        '空白',
        true,
        SelectorCommon.highlightText
    );
    assert.ok(html.includes('<mark>空白</mark>'));
    assert.ok(html.includes('選択中'));
    assert.ok(html.includes('value-hint'));
}

/** クリップ検索と画像キーワード一致ができること */
function test_clip_item_matches() {
    assert.equal(ClipSelectorLogic.itemMatches(SAMPLE_CLIPS[0], 'hello'), true);
    assert.equal(ClipSelectorLogic.itemMatches(SAMPLE_CLIPS[0], '画像'), false);
    assert.equal(ClipSelectorLogic.itemMatches(SAMPLE_CLIPS[1], '画像'), true);
}

/** 登録行インデックスと総行数を計算できること */
function test_clip_register_index_and_total_rows() {
    assert.equal(ClipSelectorLogic.isRegisterIndex(2, 2), true);
    assert.equal(ClipSelectorLogic.isRegisterIndex(1, 2), false);
    assert.equal(ClipSelectorLogic.totalRows(2), 3);
}

/** フィルタ後の選択インデックスを維持できること */
function test_clip_next_selection_after_filter() {
    // 登録行を維持
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter(2, 2, 1), 1);
    // 既存行を維持
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter(0, 2, 1), 0);
    // 件数減少で範囲外になった場合は先頭へ
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter(2, 3, 1), 0);
    // 最終アイテム選択中に件数が減っても登録行へ誤判定しない
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter(2, 3, 2), 0);
}

/** 空一覧メッセージを切り替えられること */
function test_clip_empty_list_message() {
    assert.equal(ClipSelectorLogic.emptyListMessage(0, 0), '登録クリップがありません');
    assert.equal(ClipSelectorLogic.emptyListMessage(2, 0), '該当する登録クリップがありません');
    assert.equal(ClipSelectorLogic.emptyListMessage(2, 1), null);
}

/** 画像ホバープレビュー可否とソースを解決できること */
function test_clip_image_preview() {
    assert.equal(
        ClipSelectorLogic.imagePreviewSource(SAMPLE_CLIPS[1]),
        'data:image/png;base64,hover'
    );
    assert.equal(ClipSelectorLogic.canShowImageHoverPreview(SAMPLE_CLIPS[1]), true);
    assert.equal(ClipSelectorLogic.canShowImageHoverPreview(SAMPLE_CLIPS[0]), false);
}

/** 選択確定アクションを解決できること */
function test_clip_resolve_activate_selection() {
    assert.deepEqual(ClipSelectorLogic.resolveActivateSelection(2, SAMPLE_CLIPS), {
        type: 'register',
    });
    assert.deepEqual(ClipSelectorLogic.resolveActivateSelection(0, SAMPLE_CLIPS), {
        type: 'select',
        id: '0',
    });
    assert.equal(ClipSelectorLogic.resolveActivateSelection(5, SAMPLE_CLIPS), null);
}

/** 登録クリップ IPC メッセージを組み立てられること */
function test_clip_selector_ipc_messages() {
    assert.equal(ClipSelectorLogic.selectMessage('3'), 'select:3');
    assert.equal(ClipSelectorLogic.registerMessage(), 'register');
    assert.equal(ClipSelectorLogic.deleteMessage('3'), 'delete:3');
}

/** カテゴリ見出し HTML を組み立てられること */
function test_build_category_header_inner_html() {
    assert.equal(
        SelectorCommon.buildCategoryHeaderInnerHtml('<b>', SelectorCommon.escapeHtml),
        '<span class="category-accent" aria-hidden="true"></span>' +
            '<span class="category-label">&lt;b&gt;</span>'
    );
}

/** 登録行 HTML を組み立てられること */
function test_build_register_row_inner_html() {
    const html = ClipSelectorLogic.buildRegisterRowInnerHtml('登録', SelectorCommon.highlightText);
    assert.ok(html.includes('<mark>登録</mark>'));
    assert.ok(html.includes('クリップボードを'));
}

const tests = [
    test_escape_html,
    test_normalize_query,
    test_highlight_text,
    test_clamp_index,
    test_handle_arrow_keys,
    test_handle_escape_key,
    test_favorite_ids_sorted,
    test_build_display_list_without_query,
    test_build_display_list_with_query,
    test_section_category,
    test_next_index_after_filter,
    test_can_move_selected_favorite,
    test_quick_selector_ipc_messages,
    test_build_item_inner_html,
    test_clip_item_matches,
    test_clip_register_index_and_total_rows,
    test_clip_next_selection_after_filter,
    test_clip_empty_list_message,
    test_clip_image_preview,
    test_clip_resolve_activate_selection,
    test_clip_selector_ipc_messages,
    test_build_category_header_inner_html,
    test_build_register_row_inner_html,
];

for (const test of tests) {
    test();
}

console.log(`selector_logic: ${tests.length} passed`);

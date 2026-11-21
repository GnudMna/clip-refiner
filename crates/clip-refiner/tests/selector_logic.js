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

/** 空クエリではお気に入りの後に折りたたんだカテゴリ見出しだけを並べること */
function test_build_display_list_without_query() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set());
    assert.deepEqual(
        display.map((row) => row.id),
        ['UrlDecode', 'Trim', 'category:テキスト', 'category:エンコード']
    );
    assert.equal(display[0].listSection, 'favorites');
    assert.equal(display[2].listSection, 'header');
    assert.equal(display[2].expanded, false);
    assert.equal(display[2].count, 2);
    assert.equal(display[3].count, 1);
}

/** 展開中のカテゴリだけ見出しの直後にモードを並べること */
function test_build_display_list_with_expanded_category() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set(['テキスト']));
    assert.deepEqual(
        display.map((row) => row.id),
        ['UrlDecode', 'Trim', 'category:テキスト', 'Trim', 'JsonFormat', 'category:エンコード']
    );
    assert.equal(display[2].expanded, true);
    assert.equal(display[3].listSection, 'category');
}

/** カテゴリなしのモードは見出しを持たず常に表示すること */
function test_build_display_list_uncategorized_always_visible() {
    const modes = [{ id: 'Plain', label: '無加工', category: '' }, ...SAMPLE_MODES];
    const display = QuickSelectorLogic.buildDisplayList(modes, '', new Set());
    assert.equal(display[2].id, 'Plain');
    assert.equal(display[2].listSection, 'category');
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 2), -1);
}

/** クエリでモードを絞り込み、検索中はカテゴリの開閉を無視すること */
function test_build_display_list_with_query() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'json', new Set());
    assert.equal(display.length, 1);
    assert.equal(display[0].id, 'JsonFormat');
    assert.equal(QuickSelectorLogic.isCategoryRow(display[0]), false);
}

/** 行が属するカテゴリ見出しのインデックスを求められること */
function test_category_row_index_of() {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set(['テキスト']));
    // お気に入り行は見出しに属さない
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 0), -1);
    // 見出し行は自身
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 2), 2);
    // カテゴリ内のモード行は直前の見出し
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 4), 2);
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 5), 5);
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 99), -1);
}

/** 展開カテゴリの切り替えと保存値の復元ができること */
function test_expanded_categories_state() {
    const initial = new Set(['テキスト']);
    const expanded = QuickSelectorLogic.withCategoryExpanded(initial, 'エンコード', true);
    assert.deepEqual([...expanded].sort(), ['エンコード', 'テキスト']);
    // 元の集合は変更しない
    assert.deepEqual([...initial], ['テキスト']);
    const collapsed = QuickSelectorLogic.withCategoryExpanded(expanded, 'テキスト', false);
    assert.deepEqual([...collapsed], ['エンコード']);

    assert.deepEqual([...QuickSelectorLogic.toExpandedCategories(['a', 1, 'b'])], ['a', 'b']);
    assert.equal(QuickSelectorLogic.toExpandedCategories(undefined).size, 0);
    assert.equal(QuickSelectorLogic.toExpandedCategories({ a: 1 }).size, 0);
    assert.equal(
        QuickSelectorLogic.expandedCategoriesMessage(new Set(['URL操作'])),
        'expanded-categories:["URL操作"]'
    );
}

/** カテゴリ見出しラベルを解決できること */
function test_section_category() {
    const favorite = { id: 'Trim', label: 'Trim', category: 'テキスト', listSection: 'favorites' };
    assert.equal(QuickSelectorLogic.sectionCategory(favorite, ''), 'お気に入り');
    assert.equal(QuickSelectorLogic.sectionCategory(favorite, 't'), null);
    assert.equal(
        QuickSelectorLogic.sectionCategory(
            {
                id: 'category:テキスト',
                label: 'テキスト',
                category: 'テキスト',
                listSection: 'header',
            },
            ''
        ),
        'テキスト'
    );
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
    const filtered = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'url', new Set());
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
    assert.equal(QuickSelectorLogic.applyMessage('Trim'), 'apply:Trim');
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
    assert.ok(html.includes('自動変換中'));
    assert.ok(html.includes('value-hint'));
}

/** 選択行のテンプレート切り替え要求をテキストクリップだけで返すこと */
function test_clip_resolve_template_toggle() {
    const clips = [
        ...SAMPLE_CLIPS,
        { id: '2', label: '日報', preview: '{date} 日報', kind: 'text', template: true },
    ];
    assert.deepEqual(ClipSelectorLogic.resolveTemplateToggle(0, clips), { id: '0', enabled: true });
    assert.deepEqual(ClipSelectorLogic.resolveTemplateToggle(2, clips), {
        id: '2',
        enabled: false,
    });
    // 画像・登録行 (範囲外) は対象外
    assert.equal(ClipSelectorLogic.resolveTemplateToggle(1, clips), null);
    assert.equal(ClipSelectorLogic.resolveTemplateToggle(3, clips), null);
    // テンプレートのクリップは「テンプレート」でも検索できる
    assert.equal(ClipSelectorLogic.itemMatches(clips[2], 'テンプレート'), true);
    assert.equal(ClipSelectorLogic.itemMatches(clips[0], 'テンプレート'), false);
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
    assert.equal(ClipSelectorLogic.moveMessage('3', 'up'), 'move:3:up');
    assert.equal(ClipSelectorLogic.templateMessage('3', true), 'template:3:on');
    assert.equal(ClipSelectorLogic.templateMessage('3', false), 'template:3:off');
}

/** 並べ替えの移動先を検索中・登録行・端を除いて求められること */
function test_clip_move_target_index() {
    assert.equal(ClipSelectorLogic.canReorder(''), true);
    assert.equal(ClipSelectorLogic.canReorder('a'), false);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 1, 3, 'up'), 0);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 1, 3, 'down'), 2);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 0, 3, 'up'), null);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 2, 3, 'down'), null);
    // 登録行 (index === filteredLength) は動かさない
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 3, 3, 'up'), null);
    assert.equal(ClipSelectorLogic.moveTargetIndex('a', 1, 3, 'up'), null);
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
    test_build_display_list_with_expanded_category,
    test_build_display_list_uncategorized_always_visible,
    test_build_display_list_with_query,
    test_category_row_index_of,
    test_expanded_categories_state,
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
    test_clip_resolve_template_toggle,
    test_clip_move_target_index,
    test_build_category_header_inner_html,
    test_build_register_row_inner_html,
];

for (const test of tests) {
    test();
}

console.log(`selector_logic: ${tests.length} passed`);

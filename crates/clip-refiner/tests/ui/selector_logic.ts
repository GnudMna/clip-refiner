/**
 * セレクター共有 / クイック / 登録クリップ純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';
import { test } from 'node:test';

import ClipSelectorLogic from '../../src/ui/selector/clip_selector_logic.ts';
import QuickSelectorLogic from '../../src/ui/selector/quick_selector_logic.ts';
import SelectorCommon from '../../src/ui/selector/selector_common.ts';

import type { ClipSelectorItem } from '../../src/ui/selector/clip_selector_logic.ts';
import type { QuickSelectorMode } from '../../src/ui/selector/quick_selector_logic.ts';

/**
 * ダミーのキーボードイベントを作る (`preventDefault` 呼び出しを `prevented` に記録する)
 */
function keyEvent(
    key: string,
    mods: Partial<KeyboardEvent> = {}
): KeyboardEvent & { prevented?: boolean } {
    return {
        key,
        ...mods,
        preventDefault() {
            this.prevented = true;
        },
    } as KeyboardEvent & { prevented?: boolean };
}

/**
 * 最小限のクリップ項目を作る
 */
function clipItem(id: string): ClipSelectorItem {
    return { id, label: id, kind: 'text' };
}

/**
 * 選択操作が使う分だけを備えた結果リストの代わりを作る
 */
function fakeResults(count: number): {
    results: HTMLElement;
    /** `.selected` が付いた行の位置を返す */
    selected: () => number[];
    /** スクロールされた行の位置 (呼び出し順) */
    scrolled: number[];
} {
    const scrolled: number[] = [];
    const selectedRows = new Set<number>();
    const rows = Array.from({ length: count }, (_, index) => ({
        classList: {
            toggle(name: string, force: boolean) {
                assert.equal(name, 'selected');
                if (force) {
                    selectedRows.add(index);
                } else {
                    selectedRows.delete(index);
                }
            },
        },
        scrollIntoView: () => scrolled.push(index),
    }));
    const results = {
        querySelectorAll(selector: string) {
            assert.equal(selector, '.item');
            return rows;
        },
    } as unknown as HTMLElement;
    return { results, selected: () => [...selectedRows], scrolled };
}

const SAMPLE_MODES: QuickSelectorMode[] = [
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

const SAMPLE_CLIPS: ClipSelectorItem[] = [
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

/**
 * 添字アクセスの結果が存在することを検証して返す (noUncheckedIndexedAccess 対策)
 */
function at<T>(list: readonly T[], index: number): T {
    const value = list[index];
    assert.notEqual(value, undefined, 'index ' + index + ' が存在しない');
    return value as T;
}

/** HTML 特殊文字をエスケープできること */
test('escape_html', () => {
    assert.equal(SelectorCommon.escapeHtml('<b>&"'), '&lt;b&gt;&amp;&quot;');
});

/** 検索クエリを正規化できること */
test('normalize_query', () => {
    assert.equal(SelectorCommon.normalizeQuery('  AbC  '), 'abc');
    assert.equal(SelectorCommon.normalizeQuery(null as unknown as string), '');
});

/** 一致部分をハイライトできること */
test('highlight_text', () => {
    assert.equal(SelectorCommon.highlightText('Hello', 'ell'), 'H<mark>ell</mark>o');
    assert.equal(SelectorCommon.highlightText('<b>', 'b'), '&lt;<mark>b</mark>&gt;');
    assert.equal(SelectorCommon.highlightText('Hello', ''), 'Hello');
});

/** 小文字化で長さが変わる文字があってもハイライト位置がずれないこと */
test('highlight_text_length_changing_lowercase', () => {
    // 'İ'.toLowerCase() は 'i' + U+0307 の 2 コードユニットになる
    assert.equal(SelectorCommon.highlightText('İstanbul', 'bul'), 'İstan<mark>bul</mark>');
    assert.equal(SelectorCommon.highlightText('İİ abc', 'abc'), 'İİ <mark>abc</mark>');
    // 一致が小文字化後の文字の途中で終わるときは元の文字全体を含める
    assert.equal(SelectorCommon.highlightText('aİb', 'ai'), '<mark>aİ</mark>b');
    // サロゲートペアを挟んでもずれない
    assert.equal(SelectorCommon.highlightText('İ😀x', 'x'), 'İ😀<mark>x</mark>');
});

/** 選択インデックスをクランプできること */
test('clamp_index', () => {
    assert.equal(SelectorCommon.clampIndex(5, 0), 0);
    assert.equal(SelectorCommon.clampIndex(5, 3), 2);
    assert.equal(SelectorCommon.clampIndex(-1, 3), 0);
    assert.equal(SelectorCommon.clampIndex(1, 3), 1);
});

/** 矢印キーで選択移動を処理できること */
test('handle_arrow_keys', () => {
    const moves: number[] = [];
    const event = keyEvent('ArrowDown');
    assert.equal(
        SelectorCommon.handleArrowKeys(event, (delta) => moves.push(delta)),
        true
    );
    assert.deepEqual(moves, [1]);
    assert.equal(event.prevented, true);
    assert.equal(
        SelectorCommon.handleArrowKeys(keyEvent('Enter'), () => {}),
        false
    );
});

/** Ctrl+Home / Ctrl+End だけが先頭・末尾移動になり、素の Home / End は検索欄に残ること */
test('handle_home_end_keys', () => {
    const jumps: number[] = [];
    const setSelectedIndex = (index: number) => jumps.push(index);

    const plainHome = keyEvent('Home');
    assert.equal(
        SelectorCommon.handleHomeEndKeys(plainHome, () => 5, setSelectedIndex),
        false
    );
    assert.equal(plainHome.prevented, undefined);
    assert.equal(
        SelectorCommon.handleHomeEndKeys(keyEvent('End'), () => 5, setSelectedIndex),
        false
    );
    assert.equal(
        SelectorCommon.handleHomeEndKeys(
            keyEvent('Home', { ctrlKey: true, shiftKey: true }),
            () => 5,
            setSelectedIndex
        ),
        false
    );
    assert.deepEqual(jumps, []);

    const ctrlHome = keyEvent('Home', { ctrlKey: true });
    assert.equal(
        SelectorCommon.handleHomeEndKeys(ctrlHome, () => 5, setSelectedIndex),
        true
    );
    assert.equal(ctrlHome.prevented, true);
    assert.equal(
        SelectorCommon.handleHomeEndKeys(
            keyEvent('End', { ctrlKey: true }),
            () => 5,
            setSelectedIndex
        ),
        true
    );
    assert.deepEqual(jumps, [0, 4]);
});

/** Escape でフィルタクリアまたは閉じるを処理できること */
test('handle_escape_key', () => {
    let cleared = false;
    let closed = false;
    const input = { value: 'abc' } as HTMLInputElement;
    const event = keyEvent('Escape');
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
});

/** 上矢印キーで前の行へ移動すること */
test('handle_arrow_up_key', () => {
    const moves: number[] = [];
    const event = keyEvent('ArrowUp');
    assert.equal(
        SelectorCommon.handleArrowKeys(event, (delta) => moves.push(delta)),
        true
    );
    assert.deepEqual(moves, [-1]);
    assert.equal(event.prevented, true);
});

/** Ctrl+Home / Ctrl+End は行が無ければ移動せず、ほかのキーは処理しないこと */
test('handle_home_end_keys_without_rows', () => {
    const jumps: number[] = [];
    const handle = (event: KeyboardEvent, count: number) =>
        SelectorCommon.handleHomeEndKeys(
            event,
            () => count,
            (index) => jumps.push(index)
        );

    for (const key of ['Home', 'End']) {
        const event = keyEvent(key, { ctrlKey: true });
        assert.equal(handle(event, 0), true);
        assert.equal(event.prevented, undefined);
    }
    assert.equal(handle(keyEvent('a', { ctrlKey: true }), 5), false);
    assert.deepEqual(jumps, []);
});

/** Escape 以外のキーではフィルタもウィンドウも変えないこと */
test('handle_escape_key_ignores_other_keys', () => {
    const input = { value: 'abc' } as HTMLInputElement;
    const event = keyEvent('Enter');
    const unexpected = () => assert.fail('呼ばれてはならない');
    assert.equal(SelectorCommon.handleEscapeKey(event, input, unexpected, unexpected), false);
    assert.equal(input.value, 'abc');
    assert.equal(event.prevented, undefined);
});

/** 検索入力欄にフォーカスしてから全選択すること */
test('focus_search_input', () => {
    const calls: string[] = [];
    SelectorCommon.focusSearchInput({
        focus: () => calls.push('focus'),
        select: () => calls.push('select'),
    } as unknown as HTMLInputElement);
    assert.deepEqual(calls, ['focus', 'select']);
});

/** 選択行だけをハイライトし、移動先の行を表示範囲内にスクロールすること */
test('selection_moves_highlight_and_scrolls', () => {
    const list = fakeResults(3);
    const selection = SelectorCommon.createSelection(list.results, () => 3);
    assert.equal(selection.index, 0);

    selection.moveSelection(1);
    assert.equal(selection.index, 1);
    assert.deepEqual(list.selected(), [1]);
    assert.deepEqual(list.scrolled, [1]);

    // スクロールを求めなければハイライトだけを移す
    selection.setSelectedIndex(2, false);
    assert.deepEqual(list.selected(), [2]);
    assert.deepEqual(list.scrolled, [1]);

    // 範囲外は端の行に収める
    selection.setSelectedIndex(-5, true);
    assert.equal(selection.index, 0);
    selection.setSelectedIndex(99, true);
    assert.equal(selection.index, 2);
    assert.deepEqual(list.selected(), [2]);
    assert.deepEqual(list.scrolled, [1, 0, 2]);
});

/** 選択が変わらない場合は、求められたときのスクロールだけを行うこと */
test('selection_unchanged_only_scrolls', () => {
    const list = fakeResults(2);
    const selection = SelectorCommon.createSelection(list.results, () => 2);

    selection.setSelectedIndex(0, false);
    assert.deepEqual(list.scrolled, []);
    selection.setSelectedIndex(0, true);
    assert.deepEqual(list.scrolled, [0]);
    assert.deepEqual(list.selected(), []);
});

/** 行が無ければ選択を動かさないこと */
test('selection_without_rows', () => {
    const list = fakeResults(0);
    const selection = SelectorCommon.createSelection(list.results, () => 0);

    selection.moveSelection(1);
    selection.setSelectedIndex(1, true);
    selection.scrollSelectedIntoView();
    assert.equal(selection.index, 0);
    assert.deepEqual(list.scrolled, []);
});

/** 行数が減ったときに選択を範囲内へ収められること */
test('selection_clamps_after_rows_shrink', () => {
    let count = 5;
    const list = fakeResults(5);
    const selection = SelectorCommon.createSelection(list.results, () => count);

    selection.index = 4;
    count = 2;
    selection.clampSelectedIndex();
    assert.equal(selection.index, 1);
});

/** 一致しない検索語ではハイライトしないこと */
test('highlight_text_without_match', () => {
    assert.equal(SelectorCommon.highlightText('<b>', 'x'), '&lt;b&gt;');
    // 小文字化で長さが変わる文字を含む場合
    assert.equal(SelectorCommon.highlightText('İ<b>', 'x'), 'İ&lt;b&gt;');
});

/** お気に入り ID を表示順で返すこと */
test('favorite_ids_sorted', () => {
    assert.deepEqual(QuickSelectorLogic.favoriteIds(SAMPLE_MODES), ['UrlDecode', 'Trim']);
});

/** 空クエリではお気に入りの後に折りたたんだカテゴリ見出しだけを並べること */
test('build_display_list_without_query', () => {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set());
    assert.deepEqual(
        display.map((row) => row.id),
        ['UrlDecode', 'Trim', 'category:テキスト', 'category:エンコード']
    );
    assert.equal(at(display, 0).listSection, 'favorites');
    assert.equal(at(display, 2).listSection, 'header');
    assert.equal(at(display, 2).expanded, false);
    assert.equal(at(display, 2).count, 2);
    assert.equal(at(display, 3).count, 1);
});

/** 展開中のカテゴリだけ見出しの直後にモードを並べること */
test('build_display_list_with_expanded_category', () => {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set(['テキスト']));
    assert.deepEqual(
        display.map((row) => row.id),
        ['UrlDecode', 'Trim', 'category:テキスト', 'Trim', 'JsonFormat', 'category:エンコード']
    );
    assert.equal(at(display, 2).expanded, true);
    assert.equal(at(display, 3).listSection, 'category');
});

/** カテゴリなしのモードは見出しを持たず常に表示すること */
test('build_display_list_uncategorized_always_visible', () => {
    const modes = [{ id: 'Plain', label: '無加工', category: '' }, ...SAMPLE_MODES];
    const display = QuickSelectorLogic.buildDisplayList(modes, '', new Set());
    assert.equal(at(display, 2).id, 'Plain');
    assert.equal(at(display, 2).listSection, 'category');
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 2), -1);
});

/** クエリでモードを絞り込み、検索中はカテゴリの開閉を無視すること */
test('build_display_list_with_query', () => {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'json', new Set());
    assert.equal(display.length, 1);
    assert.equal(at(display, 0).id, 'JsonFormat');
    assert.equal(QuickSelectorLogic.isCategoryRow(display[0]), false);
});

/** 行が属するカテゴリ見出しのインデックスを求められること */
test('category_row_index_of', () => {
    const display = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, '', new Set(['テキスト']));
    // お気に入り行は見出しに属さない
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 0), -1);
    // 見出し行は自身
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 2), 2);
    // カテゴリ内のモード行は直前の見出し
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 4), 2);
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 5), 5);
    assert.equal(QuickSelectorLogic.categoryRowIndexOf(display, 99), -1);
});

/** 展開カテゴリの切り替えと保存値の復元ができること */
test('expanded_categories_state', () => {
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
});

/** カテゴリ見出しラベルを解決できること */
test('section_category', () => {
    const favorite: QuickSelectorMode = {
        id: 'Trim',
        label: 'Trim',
        category: 'テキスト',
        listSection: 'favorites',
    };
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
});

/** フィルタ後の選択インデックスを維持できること */
test('next_index_after_filter', () => {
    const filtered = QuickSelectorLogic.buildDisplayList(SAMPLE_MODES, 'url', new Set());
    const urlDecode = filtered.find((mode) => mode.id === 'UrlDecode');
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, urlDecode), 0);
    assert.equal(
        QuickSelectorLogic.nextIndexAfterFilter(filtered, {
            id: 'missing',
            label: 'missing',
            category: '',
        }),
        0
    );
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, undefined), 0);
});

/** 同じ id がお気に入り節とカテゴリ節に並ぶとき、選択した節の行を維持すること */
test('next_index_after_filter_keeps_section', () => {
    const filtered: QuickSelectorMode[] = [
        { id: 'Trim', label: 'Trim', category: '', listSection: 'favorites', favorite: true },
        { id: 'cat:テキスト', label: 'テキスト', category: 'テキスト', listSection: 'header' },
        { id: 'Trim', label: 'Trim', category: 'テキスト', listSection: 'category' },
    ];
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, filtered[2]), 2);
    assert.equal(QuickSelectorLogic.nextIndexAfterFilter(filtered, filtered[0]), 0);
    // 同じ節に無ければ id だけで探す
    assert.equal(
        QuickSelectorLogic.nextIndexAfterFilter(filtered.slice(1), {
            id: 'Trim',
            label: 'Trim',
            category: '',
            listSection: 'favorites',
        }),
        1
    );
});

/** お気に入り並べ替え可否を判定できること */
test('can_move_selected_favorite', () => {
    const favorite: QuickSelectorMode = {
        id: 'Trim',
        label: 'Trim',
        category: 'テキスト',
        favorite: true,
        listSection: 'favorites',
    };
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('', favorite), true);
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('t', favorite), false);
    assert.equal(QuickSelectorLogic.canMoveSelectedFavorite('', undefined), false);
});

/** クイックセレクター IPC メッセージを組み立てられること */
test('quick_selector_ipc_messages', () => {
    assert.equal(QuickSelectorLogic.applyMessage('Trim'), 'apply:Trim');
    assert.equal(QuickSelectorLogic.selectMessage('Trim'), 'select:Trim');
    assert.equal(QuickSelectorLogic.toggleFavoriteMessage('Trim'), 'toggle-favorite:Trim');
    assert.equal(QuickSelectorLogic.moveFavoriteMessage('Trim', 'up'), 'move-favorite:Trim:up');
});

/** モード行 HTML を組み立てられること */
test('build_item_inner_html', () => {
    const html = QuickSelectorLogic.buildItemInnerHtml(
        at(SAMPLE_MODES, 0),
        '空白',
        true,
        SelectorCommon.highlightText
    );
    assert.ok(html.includes('<mark>空白</mark>'));
    assert.ok(html.includes('自動変換中'));
    assert.ok(html.includes('value-hint'));
});

/** 選択行のテンプレート切り替え要求をテキストクリップだけで返すこと */
test('clip_resolve_template_toggle', () => {
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
    assert.equal(ClipSelectorLogic.itemMatches(at(clips, 2), 'テンプレート'), true);
    assert.equal(ClipSelectorLogic.itemMatches(at(clips, 0), 'テンプレート'), false);
});

/** クリップ検索と画像キーワード一致ができること */
test('clip_item_matches', () => {
    assert.equal(ClipSelectorLogic.itemMatches(at(SAMPLE_CLIPS, 0), 'hello'), true);
    assert.equal(ClipSelectorLogic.itemMatches(at(SAMPLE_CLIPS, 0), '画像'), false);
    assert.equal(ClipSelectorLogic.itemMatches(at(SAMPLE_CLIPS, 1), '画像'), true);
});

/** 登録行インデックスと総行数を計算できること */
test('clip_register_index_and_total_rows', () => {
    assert.equal(ClipSelectorLogic.isRegisterIndex(2, 2), true);
    assert.equal(ClipSelectorLogic.isRegisterIndex(1, 2), false);
    assert.equal(ClipSelectorLogic.totalRows(2), 3);
});

/** フィルタ後の選択インデックスを維持できること */
test('clip_next_selection_after_filter', () => {
    const a = clipItem('a');
    const b = clipItem('b');
    const c = clipItem('c');
    // 登録行を維持
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter(null, [a]), 1);
    // 同じクリップを ID で追跡する (位置が変わっても維持)
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter('c', [c]), 0);
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter('b', [a, b, c]), 1);
    // 結果から消えた場合は先頭へ
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter('b', [a, c]), 0);
    assert.equal(ClipSelectorLogic.nextSelectionAfterFilter('b', []), 0);
});

/** フィルタ前の選択行から ID を求められること */
test('clip_selected_item_id', () => {
    const filtered = [clipItem('a'), clipItem('b')];
    assert.equal(ClipSelectorLogic.selectedItemId(1, filtered), 'b');
    // 登録行・範囲外は null
    assert.equal(ClipSelectorLogic.selectedItemId(2, filtered), null);
    assert.equal(ClipSelectorLogic.selectedItemId(0, []), null);
});

/** 空一覧メッセージを切り替えられること */
test('clip_empty_list_message', () => {
    assert.equal(ClipSelectorLogic.emptyListMessage(0, 0), '登録クリップがありません');
    assert.equal(ClipSelectorLogic.emptyListMessage(2, 0), '該当する登録クリップがありません');
    assert.equal(ClipSelectorLogic.emptyListMessage(2, 1), null);
});

/** 画像ホバープレビュー可否とソースを解決できること */
test('clip_image_preview', () => {
    assert.equal(
        ClipSelectorLogic.imagePreviewSource(at(SAMPLE_CLIPS, 1)),
        'data:image/png;base64,hover'
    );
    assert.equal(ClipSelectorLogic.canShowImageHoverPreview(at(SAMPLE_CLIPS, 1)), true);
    assert.equal(ClipSelectorLogic.canShowImageHoverPreview(at(SAMPLE_CLIPS, 0)), false);
});

/** 選択確定アクションを解決できること */
test('clip_resolve_activate_selection', () => {
    assert.deepEqual(ClipSelectorLogic.resolveActivateSelection(2, SAMPLE_CLIPS), {
        type: 'register',
    });
    assert.deepEqual(ClipSelectorLogic.resolveActivateSelection(0, SAMPLE_CLIPS), {
        type: 'select',
        id: '0',
    });
    assert.equal(ClipSelectorLogic.resolveActivateSelection(5, SAMPLE_CLIPS), null);
});

/** 登録クリップ IPC メッセージを組み立てられること */
test('clip_selector_ipc_messages', () => {
    assert.equal(ClipSelectorLogic.selectMessage('3'), 'select:3');
    assert.equal(ClipSelectorLogic.registerMessage(), 'register');
    assert.equal(ClipSelectorLogic.deleteMessage('3'), 'delete:3');
    assert.equal(ClipSelectorLogic.moveMessage('3', 'up'), 'move:3:up');
    assert.equal(ClipSelectorLogic.templateMessage('3', true), 'template:3:on');
    assert.equal(ClipSelectorLogic.templateMessage('3', false), 'template:3:off');
    assert.equal(ClipSelectorLogic.counterResetMessage('3'), 'counter-reset:3');
});

/** 連番のリセット要求と表示を、連番を払い出し済みのテキストクリップだけで返すこと */
test('clip_counter_reset_and_badge', () => {
    const clips = [
        {
            id: '0',
            label: '請求書',
            preview: 'No.{counter}',
            kind: 'text',
            template: true,
            counter: 4,
        },
        {
            id: '1',
            label: '未使用',
            preview: '{counter}',
            kind: 'text',
            template: true,
            counter: 0,
        },
        { id: '2', label: '画像', preview: '', kind: 'image', counter: 0 },
    ];
    assert.deepEqual(ClipSelectorLogic.resolveCounterReset(0, clips), { id: '0' });
    // 未使用・画像・登録行 (範囲外) は対象外
    assert.equal(ClipSelectorLogic.resolveCounterReset(1, clips), null);
    assert.equal(ClipSelectorLogic.resolveCounterReset(2, clips), null);
    assert.equal(ClipSelectorLogic.resolveCounterReset(3, clips), null);
    assert.equal(ClipSelectorLogic.counterBadgeText(at(clips, 0)), '連番 4');
    assert.equal(ClipSelectorLogic.counterBadgeText(at(clips, 1)), '');
    assert.equal(ClipSelectorLogic.counterBadgeText({ id: '9', label: 'x', kind: 'text' }), '');
});

/** 全文ホバー表示の要求対象と表示内容を解決できること */
test('clip_full_text_hover', () => {
    assert.equal(ClipSelectorLogic.fullTextMessage('3'), 'full:3');
    // 画像は対象外
    assert.equal(ClipSelectorLogic.canRequestFullText(at(SAMPLE_CLIPS, 0)), true);
    assert.equal(ClipSelectorLogic.canRequestFullText(at(SAMPLE_CLIPS, 1)), false);
    // 空・文字列以外は表示しない
    assert.equal(ClipSelectorLogic.fullTextToShow(''), null);
    assert.equal(ClipSelectorLogic.fullTextToShow(null), null);
    // 一覧のプレビューと同じ内容でも表示する
    assert.equal(ClipSelectorLogic.fullTextToShow('hello world'), 'hello world');
    assert.equal(
        ClipSelectorLogic.fullTextToShow('hello world\nsecond line'),
        'hello world\nsecond line'
    );
});

/** 並べ替えの移動先を検索中・登録行・端を除いて求められること */
test('clip_move_target_index', () => {
    assert.equal(ClipSelectorLogic.canReorder(''), true);
    assert.equal(ClipSelectorLogic.canReorder('a'), false);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 1, 3, 'up'), 0);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 1, 3, 'down'), 2);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 0, 3, 'up'), null);
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 2, 3, 'down'), null);
    // 登録行 (index === filteredLength) は動かさない
    assert.equal(ClipSelectorLogic.moveTargetIndex('', 3, 3, 'up'), null);
    assert.equal(ClipSelectorLogic.moveTargetIndex('a', 1, 3, 'up'), null);
});

/** 削除要求は確認待ちの行への 2 回目だけが削除になること */
test('clip_resolve_delete_request', () => {
    assert.equal(ClipSelectorLogic.resolveDeleteRequest(null, '1'), 'confirm');
    assert.equal(ClipSelectorLogic.resolveDeleteRequest('2', '1'), 'confirm');
    assert.equal(ClipSelectorLogic.resolveDeleteRequest('1', '1'), 'delete');
});

/** Del キーは検索欄に文字があれば文字削除に回し、リピートは無視すること */
test('clip_resolve_delete_key', () => {
    assert.equal(ClipSelectorLogic.resolveDeleteKey('', false), 'delete');
    assert.equal(ClipSelectorLogic.resolveDeleteKey('', true), 'ignore');
    assert.equal(ClipSelectorLogic.resolveDeleteKey('abc', false), 'text');
    assert.equal(ClipSelectorLogic.resolveDeleteKey('abc', true), 'text');
    // 空白だけでも検索欄の文字として扱う
    assert.equal(ClipSelectorLogic.resolveDeleteKey(' ', false), 'text');
});

/** カテゴリ見出し HTML を組み立てられること */
test('build_category_header_inner_html', () => {
    assert.equal(
        SelectorCommon.buildCategoryHeaderInnerHtml('<b>', SelectorCommon.escapeHtml),
        '<span class="category-accent" aria-hidden="true"></span>' +
            '<span class="category-label">&lt;b&gt;</span>'
    );
});

/** 登録行 HTML を組み立てられること */
test('build_register_row_inner_html', () => {
    const html = ClipSelectorLogic.buildRegisterRowInnerHtml('登録', SelectorCommon.highlightText);
    assert.ok(html.includes('<mark>登録</mark>'));
    assert.ok(html.includes('クリップボードを'));
});

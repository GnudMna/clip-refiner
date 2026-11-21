/**
 * @typedef {Object} QuickSelectorMode
 * @property {string} id
 * @property {string} label
 * @property {string} category
 * @property {string} [value]
 * @property {boolean} [favorite]
 * @property {number} [favoriteIndex]
 * @property {boolean} [producesImage] - 画像を出力するモードか
 * @property {'favorites'|'category'|'header'} [listSection] - `header` はカテゴリ見出し行
 * @property {boolean} [expanded] - 見出し行のカテゴリが展開中かどうか
 * @property {number} [count] - 見出し行のカテゴリに属するモード数
 */

/**
 * クイックセレクターの DOM 非依存ロジック
 *
 * @namespace QuickSelectorLogic
 */
const QuickSelectorLogic = (function () {
    'use strict';

    /** @type {string} お気に入りセクションの表示名 */
    const FAVORITES_CATEGORY = 'お気に入り';

    /** @type {string} カテゴリ見出し行の ID 接頭辞 (モード ID と衝突しない) */
    const CATEGORY_ROW_PREFIX = 'category:';

    /**
     * ID からモードを取得する
     *
     * @param {QuickSelectorMode[]} modes
     * @param {string} id
     * @returns {QuickSelectorMode|undefined}
     */
    function modeById(modes, id) {
        return modes.find((mode) => mode.id === id);
    }

    /**
     * お気に入りモード ID を表示順で返す
     *
     * @param {QuickSelectorMode[]} modes
     * @returns {string[]}
     */
    function favoriteIds(modes) {
        return modes
            .filter((mode) => mode.favorite)
            .sort((a, b) => {
                const left = a.favoriteIndex ?? Number.MAX_SAFE_INTEGER;
                const right = b.favoriteIndex ?? Number.MAX_SAFE_INTEGER;
                return left - right;
            })
            .map((mode) => mode.id);
    }

    /**
     * 検索クエリにモードが一致するかどうか
     *
     * @param {QuickSelectorMode} mode
     * @param {string} query - 正規化済みクエリ
     * @returns {boolean}
     */
    function modeMatches(mode, query) {
        if (!query) {
            return true;
        }

        const fields = [mode.label, mode.category, mode.value, mode.id];
        return fields.some((field) => field && field.toLowerCase().includes(query));
    }

    /**
     * カテゴリ見出し行の ID を返す
     *
     * @param {string} category
     * @returns {string}
     */
    function categoryRowId(category) {
        return CATEGORY_ROW_PREFIX + category;
    }

    /**
     * 行がカテゴリ見出しかどうか
     *
     * @param {QuickSelectorMode|undefined|null} row
     * @returns {boolean}
     */
    function isCategoryRow(row) {
        return !!row && row.listSection === 'header';
    }

    /**
     * 検索クエリに応じた表示用の行一覧を組み立てる
     *
     * 未検索時はお気に入り・カテゴリなしのモード・カテゴリ見出しの順に並べ、
     * 展開中のカテゴリだけ見出しの直後にモードを並べる。検索中はカテゴリを無視して一致したモードだけを返す
     *
     * @param {QuickSelectorMode[]} modes
     * @param {string} query - 正規化済みクエリ
     * @param {Set<string>} expandedCategories - 展開中のカテゴリ名
     * @returns {QuickSelectorMode[]}
     */
    function buildDisplayList(modes, query, expandedCategories) {
        if (query) {
            return modes.filter((mode) => modeMatches(mode, query));
        }

        /** @type {QuickSelectorMode[]} */
        const display = [];
        for (const id of favoriteIds(modes)) {
            const mode = modeById(modes, id);
            if (mode) {
                display.push({ ...mode, listSection: 'favorites' });
            }
        }

        /** @type {Map<string, QuickSelectorMode[]>} */
        const byCategory = new Map();
        for (const mode of modes) {
            if (!mode.category) {
                display.push({ ...mode, listSection: 'category' });
                continue;
            }
            let members = byCategory.get(mode.category);
            if (!members) {
                members = [];
                byCategory.set(mode.category, members);
            }
            members.push(mode);
        }

        for (const [category, members] of byCategory) {
            const expanded = expandedCategories.has(category);
            display.push({
                id: categoryRowId(category),
                label: category,
                category,
                listSection: 'header',
                expanded,
                count: members.length,
            });
            if (expanded) {
                for (const mode of members) {
                    display.push({ ...mode, listSection: 'category' });
                }
            }
        }
        return display;
    }

    /**
     * 指定行が属するカテゴリ見出し行のインデックスを返す
     *
     * @param {QuickSelectorMode[]} rows
     * @param {number} index
     * @returns {number} 見出しに属さない行 (お気に入り・カテゴリなし・検索結果) は `-1`
     */
    function categoryRowIndexOf(rows, index) {
        const row = rows[index];
        if (!row || !row.category || row.listSection === 'favorites') {
            return -1;
        }
        if (isCategoryRow(row)) {
            return index;
        }
        if (row.listSection !== 'category') {
            return -1;
        }
        for (let i = index - 1; i >= 0; i--) {
            if (isCategoryRow(rows[i])) {
                return rows[i].category === row.category ? i : -1;
            }
        }
        return -1;
    }

    /**
     * 開閉状態を切り替えた後の展開カテゴリ集合を返す
     *
     * @param {Set<string>} expandedCategories
     * @param {string} category
     * @param {boolean} expand
     * @returns {Set<string>}
     */
    function withCategoryExpanded(expandedCategories, category, expand) {
        const next = new Set(expandedCategories);
        if (expand) {
            next.add(category);
        } else {
            next.delete(category);
        }
        return next;
    }

    /**
     * アプリ側から渡された展開カテゴリを集合へ変換する
     *
     * @param {unknown} categories
     * @returns {Set<string>} 配列でない値や文字列以外の要素は無視する
     */
    function toExpandedCategories(categories) {
        return new Set(
            Array.isArray(categories) ? categories.filter((value) => typeof value === 'string') : []
        );
    }

    /**
     * 展開中カテゴリの通知 IPC メッセージを組み立てる
     *
     * @param {Set<string>} expandedCategories
     * @returns {string}
     */
    function expandedCategoriesMessage(expandedCategories) {
        return 'expanded-categories:' + JSON.stringify([...expandedCategories]);
    }

    /**
     * カテゴリ見出し用のラベルを返す
     *
     * @param {QuickSelectorMode} mode
     * @param {string} query
     * @returns {string|null}
     */
    function sectionCategory(mode, query) {
        if (query) {
            return null;
        }
        if (mode.listSection === 'favorites') {
            return FAVORITES_CATEGORY;
        }
        if (mode.listSection === 'header') {
            return mode.category;
        }
        return mode.category || '';
    }

    /**
     * フィルタ後に維持する選択インデックスを求める
     *
     * @param {QuickSelectorMode[]} filtered
     * @param {string|undefined|null} previousId
     * @returns {number}
     */
    function nextIndexAfterFilter(filtered, previousId) {
        const nextIndex = previousId ? filtered.findIndex((mode) => mode.id === previousId) : -1;
        return nextIndex !== -1 ? nextIndex : 0;
    }

    /**
     * お気に入り並べ替えボタンを表示するかどうか
     *
     * @param {QuickSelectorMode} mode
     * @param {string} query
     * @returns {boolean}
     */
    function shouldShowReorderButtons(mode, query) {
        return !query && mode.listSection === 'favorites' && !!mode.favorite;
    }

    /**
     * 選択中のお気に入りを上下移動できるかどうか
     *
     * @param {string} query - 正規化済みクエリ
     * @param {QuickSelectorMode|undefined} mode
     * @returns {boolean}
     */
    function canMoveSelectedFavorite(query, mode) {
        if (query) {
            return false;
        }
        return !!(mode && mode.listSection === 'favorites' && mode.favorite);
    }

    /**
     * モード行の内部 HTML を組み立てる
     *
     * @param {QuickSelectorMode} mode
     * @param {string} query
     * @param {boolean} isCurrent
     * @param {(text: string, query: string) => string} highlightText
     * @returns {string}
     */
    function buildItemInnerHtml(mode, query, isCurrent, highlightText) {
        return (
            '<div class="mode-info">' +
            '<span class="label">' +
            highlightText(mode.label, query) +
            '</span>' +
            (isCurrent ? '<span class="current-badge">自動変換中</span>' : '') +
            '</div>' +
            (query && mode.value
                ? '<span class="value-hint">' + highlightText(mode.value, query) + '</span>'
                : '')
        );
    }

    /**
     * 現在のクリップボードへ一回だけ適用する IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function applyMessage(id) {
        return 'apply:' + id;
    }

    /**
     * 自動変換モードに設定する IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function selectMessage(id) {
        return 'select:' + id;
    }

    /**
     * お気に入りトグル IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function toggleFavoriteMessage(id) {
        return 'toggle-favorite:' + id;
    }

    /**
     * お気に入り並べ替え IPC メッセージを組み立てる
     *
     * @param {string} id
     * @param {'up'|'down'} direction
     * @returns {string}
     */
    function moveFavoriteMessage(id, direction) {
        return 'move-favorite:' + id + ':' + direction;
    }

    return {
        FAVORITES_CATEGORY,
        categoryRowId,
        isCategoryRow,
        categoryRowIndexOf,
        withCategoryExpanded,
        toExpandedCategories,
        expandedCategoriesMessage,
        modeById,
        favoriteIds,
        modeMatches,
        buildDisplayList,
        sectionCategory,
        nextIndexAfterFilter,
        shouldShowReorderButtons,
        canMoveSelectedFavorite,
        buildItemInnerHtml,
        applyMessage,
        selectMessage,
        toggleFavoriteMessage,
        moveFavoriteMessage,
    };
})();

export default QuickSelectorLogic;

/**
 * クイックセレクターの DOM 非依存ロジック
 *
 * @namespace QuickSelectorLogic
 */
const QuickSelectorLogic = (function () {
    'use strict';

    /** @type {string} お気に入りセクションの表示名 */
    const FAVORITES_CATEGORY = 'お気に入り';

    /**
     * @typedef {Object} QuickSelectorMode
     * @property {string} id
     * @property {string} label
     * @property {string} category
     * @property {string} [value]
     * @property {boolean} [favorite]
     * @property {number} [favoriteIndex]
     * @property {'favorites'|'category'} [listSection]
     */

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
     * 検索クエリに応じた表示用モード一覧を組み立てる
     *
     * @param {QuickSelectorMode[]} modes
     * @param {string} query - 正規化済みクエリ
     * @returns {QuickSelectorMode[]}
     */
    function buildDisplayList(modes, query) {
        if (query) {
            return modes.filter((mode) => modeMatches(mode, query));
        }

        const display = [];
        for (const id of favoriteIds(modes)) {
            const mode = modeById(modes, id);
            if (mode) {
                display.push(Object.assign({}, mode, { listSection: 'favorites' }));
            }
        }
        for (const mode of modes) {
            display.push(Object.assign({}, mode, { listSection: 'category' }));
        }
        return display;
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
            (isCurrent ? '<span class="current-badge">選択中</span>' : '') +
            '</div>' +
            (query && mode.value
                ? '<span class="value-hint">' + highlightText(mode.value, query) + '</span>'
                : '')
        );
    }

    /**
     * モード選択 IPC メッセージを組み立てる
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
        modeById,
        favoriteIds,
        modeMatches,
        buildDisplayList,
        sectionCategory,
        nextIndexAfterFilter,
        shouldShowReorderButtons,
        canMoveSelectedFavorite,
        buildItemInnerHtml,
        selectMessage,
        toggleFavoriteMessage,
        moveFavoriteMessage,
    };
})();

export default QuickSelectorLogic;

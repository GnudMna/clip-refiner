/**
 * 登録クリップセレクターの DOM 非依存ロジック
 *
 * @namespace ClipSelectorLogic
 */
const ClipSelectorLogic = (function () {
    'use strict';

    /**
     * @typedef {Object} ClipSelectorItem
     * @property {string} id
     * @property {string} label
     * @property {string} [preview]
     * @property {string} kind
     * @property {string} [thumbnail]
     * @property {string} [hover_preview]
     */

    /**
     * 画像プレビュー用の URL を取得する
     *
     * @param {{ hover_preview?: string, thumbnail?: string }} item
     * @returns {string|null}
     */
    function imagePreviewSource(item) {
        return item.hover_preview || item.thumbnail || null;
    }

    /**
     * 検索クエリにクリップが一致するかどうか
     *
     * @param {ClipSelectorItem} item
     * @param {string} query - 正規化済みクエリ
     * @returns {boolean}
     */
    function itemMatches(item, query) {
        if (!query) {
            return true;
        }

        const fields = [item.label, item.preview, item.kind === 'image' ? '画像' : ''];
        return fields.some((field) => field && field.toLowerCase().includes(query));
    }

    /**
     * 行インデックスが「クリップボードを登録」行かどうか
     *
     * @param {number} index
     * @param {number} filteredLength
     * @returns {boolean}
     */
    function isRegisterIndex(index, filteredLength) {
        return index === filteredLength;
    }

    /**
     * 一覧行数 (登録行を含む) を返す
     *
     * @param {number} filteredLength
     * @returns {number}
     */
    function totalRows(filteredLength) {
        return filteredLength + 1;
    }

    /**
     * フィルタ後に維持する選択インデックスを求める
     *
     * @param {number} previousIndex
     * @param {number} oldFilteredLength - フィルタ前の件数
     * @param {number} newFilteredLength - フィルタ後の件数
     * @returns {number}
     */
    function nextSelectionAfterFilter(previousIndex, oldFilteredLength, newFilteredLength) {
        if (isRegisterIndex(previousIndex, oldFilteredLength)) {
            return newFilteredLength;
        }
        if (previousIndex < newFilteredLength) {
            return previousIndex;
        }
        return 0;
    }

    /**
     * 空一覧時のメッセージを返す
     *
     * @param {number} totalItems
     * @param {number} filteredLength
     * @returns {string|null} 空でない場合は `null`
     */
    function emptyListMessage(totalItems, filteredLength) {
        if (filteredLength > 0) {
            return null;
        }
        return totalItems === 0 ? '登録クリップがありません' : '該当する登録クリップがありません';
    }

    /**
     * 画像ホバープレビューを表示できるかどうか
     *
     * @param {ClipSelectorItem} item
     * @returns {boolean}
     */
    function canShowImageHoverPreview(item) {
        return item.kind === 'image' && !!imagePreviewSource(item);
    }

    /**
     * 選択確定時のアクションを返す
     *
     * @param {number} selectedIndex
     * @param {ClipSelectorItem[]} filtered
     * @returns {{ type: 'register' }|{ type: 'select', id: string }|null}
     */
    function resolveActivateSelection(selectedIndex, filtered) {
        if (isRegisterIndex(selectedIndex, filtered.length)) {
            return { type: 'register' };
        }
        const item = filtered[selectedIndex];
        if (!item) {
            return null;
        }
        return { type: 'select', id: item.id };
    }

    /**
     * クリップ選択 IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function selectMessage(id) {
        return 'select:' + id;
    }

    /**
     * 登録 IPC メッセージを組み立てる
     *
     * @returns {string}
     */
    function registerMessage() {
        return 'register';
    }

    /**
     * 削除 IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function deleteMessage(id) {
        return 'delete:' + id;
    }

    /**
     * 「クリップボードを登録」行の内部 HTML を組み立てる
     *
     * @param {string} query
     * @param {(text: string, query: string) => string} highlightText
     * @returns {string}
     */
    function buildRegisterRowInnerHtml(query, highlightText) {
        const label = 'クリップボードを登録';
        const preview = '現在のクリップボードを登録クリップに追加';
        return (
            '<div class="mode-info">' +
            '<span class="label">' +
            highlightText(label, query) +
            '</span>' +
            '</div>' +
            '<span class="value-hint">' +
            highlightText(preview, query) +
            '</span>'
        );
    }

    return {
        imagePreviewSource,
        itemMatches,
        isRegisterIndex,
        totalRows,
        nextSelectionAfterFilter,
        emptyListMessage,
        canShowImageHoverPreview,
        resolveActivateSelection,
        selectMessage,
        registerMessage,
        deleteMessage,
        buildRegisterRowInnerHtml,
    };
})();

export default ClipSelectorLogic;

/**
 * @typedef {Object} ClipSelectorItem
 * @property {string} id
 * @property {string} label
 * @property {string} [preview]
 * @property {string} kind
 * @property {string} [token] - 識別トークン (16 桁の 16 進数。IPC の操作対象の照合に使う)
 * @property {boolean} [template] - コピー時にプレースホルダを展開するか
 * @property {number} [counter] - `{counter}` で最後に払い出した連番 (未使用は 0)
 * @property {string} [thumbnail]
 * @property {string} [hover_preview]
 */

/**
 * 登録クリップセレクターの DOM 非依存ロジック
 *
 * @namespace ClipSelectorLogic
 */
const ClipSelectorLogic = (function () {
    'use strict';

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

        const fields = [
            item.label,
            item.preview,
            item.kind === 'image' ? '画像' : '',
            item.template ? 'テンプレート' : '',
        ];
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
     * 並べ替え IPC メッセージを組み立てる
     *
     * @param {string} id
     * @param {'up'|'down'} direction
     * @returns {string}
     */
    function moveMessage(id, direction) {
        return 'move:' + id + ':' + direction;
    }

    /**
     * テンプレート切り替え IPC メッセージを組み立てる
     *
     * @param {string} id
     * @param {boolean} enabled
     * @returns {string}
     */
    function templateMessage(id, enabled) {
        return 'template:' + id + ':' + (enabled ? 'on' : 'off');
    }

    /**
     * 選択行のテンプレート展開を切り替えたときの要求内容を返す
     *
     * @param {number} selectedIndex
     * @param {ClipSelectorItem[]} filtered
     * @returns {{ id: string, enabled: boolean }|null} 登録行・画像・範囲外は `null`
     */
    function resolveTemplateToggle(selectedIndex, filtered) {
        const item = filtered[selectedIndex];
        if (!item || item.kind === 'image') {
            return null;
        }
        return { id: item.id, enabled: !item.template };
    }

    /**
     * 連番リセット IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function counterResetMessage(id) {
        return 'counter-reset:' + id;
    }

    /**
     * 選択行の連番をリセットするときの要求内容を返す
     *
     * @param {number} selectedIndex
     * @param {ClipSelectorItem[]} filtered
     * @returns {{ id: string }|null} 連番が未使用のクリップ・画像・登録行・範囲外は `null`
     */
    function resolveCounterReset(selectedIndex, filtered) {
        const item = filtered[selectedIndex];
        if (!item || item.kind === 'image' || !item.counter) {
            return null;
        }
        return { id: item.id };
    }

    /**
     * 連番バッジの表示文字列を返す
     *
     * @param {ClipSelectorItem} item
     * @returns {string} 連番が未使用のときは空文字
     */
    function counterBadgeText(item) {
        return (item.counter ?? 0) > 0 ? '連番 ' + item.counter : '';
    }

    /**
     * 全文取得 IPC メッセージを組み立てる
     *
     * @param {string} id
     * @returns {string}
     */
    function fullTextMessage(id) {
        return 'full:' + id;
    }

    /**
     * 全文のホバー表示を要求できるクリップかどうか
     *
     * @param {ClipSelectorItem} item
     * @returns {boolean}
     */
    function canRequestFullText(item) {
        return item.kind !== 'image';
    }

    /**
     * ホバー表示する全文を求める
     *
     * 一覧のプレビューと同じ内容でも表示する (一覧では改行が潰れ、見切れもあるため)
     *
     * @param {unknown} text - Rust から受け取った全文
     * @returns {string|null} 表示しない場合は `null`
     */
    function fullTextToShow(text) {
        if (typeof text !== 'string' || !text) {
            return null;
        }
        return text;
    }

    /**
     * 並べ替えボタンを表示するかどうか
     *
     * 検索中は表示順と保存順が一致しないため並べ替えを無効にする
     *
     * @param {string} query - 正規化済みクエリ
     * @returns {boolean}
     */
    function canReorder(query) {
        return !query;
    }

    /**
     * 選択行を指定方向へ移動したときの移動先インデックスを返す
     *
     * @param {string} query - 正規化済みクエリ
     * @param {number} index - 選択中の行インデックス
     * @param {number} filteredLength
     * @param {'up'|'down'} direction
     * @returns {number|null} 移動できない場合 (検索中・登録行・端) は `null`
     */
    function moveTargetIndex(query, index, filteredLength, direction) {
        if (!canReorder(query) || index < 0 || index >= filteredLength) {
            return null;
        }
        const target = direction === 'up' ? index - 1 : index + 1;
        if (target < 0 || target >= filteredLength) {
            return null;
        }
        return target;
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
        moveMessage,
        templateMessage,
        fullTextMessage,
        canRequestFullText,
        fullTextToShow,
        resolveTemplateToggle,
        counterResetMessage,
        resolveCounterReset,
        counterBadgeText,
        canReorder,
        moveTargetIndex,
        buildRegisterRowInnerHtml,
    };
})();

export default ClipSelectorLogic;

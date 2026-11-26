export interface ClipSelectorItem {
    id: string;
    label: string;
    preview?: string;
    kind: string;
    /** 識別トークン (16 桁の 16 進数。IPC の操作対象の照合に使う) */
    token?: string;
    /** コピー時にプレースホルダを展開するか */
    template?: boolean;
    /** `{counter}` で最後に払い出した連番 (未使用は 0) */
    counter?: number;
    thumbnail?: string;
    hover_preview?: string;
}

/**
 * 登録クリップセレクターの DOM 非依存ロジック
 */
const ClipSelectorLogic = (function () {
    'use strict';

    /**
     * 画像プレビュー用の URL を取得する
     */
    function imagePreviewSource(item: {
        hover_preview?: string;
        thumbnail?: string;
    }): string | null {
        return item.hover_preview || item.thumbnail || null;
    }

    /**
     * 検索クエリにクリップが一致するかどうか
     *
     * @param query - 正規化済みクエリ
     */
    function itemMatches(item: ClipSelectorItem, query: string): boolean {
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
     */
    function isRegisterIndex(index: number, filteredLength: number): boolean {
        return index === filteredLength;
    }

    /**
     * 一覧行数 (登録行を含む) を返す
     */
    function totalRows(filteredLength: number): number {
        return filteredLength + 1;
    }

    /**
     * フィルタ前に選択していた行の ID を求める
     *
     * @param index - フィルタ前の選択インデックス
     * @param filtered - フィルタ前の一覧
     * @returns 登録行・範囲外は `null`
     */
    function selectedItemId(index: number, filtered: ClipSelectorItem[]): string | null {
        return filtered[index]?.id ?? null;
    }

    /**
     * フィルタ後に維持する選択インデックスを求める
     *
     * 位置ではなく ID で追跡し、残っていなければ先頭に戻す (クイックセレクターと同じ)
     *
     * @param previousId - フィルタ前に選択していたクリップ ID (登録行は `null`)
     * @param newFiltered - フィルタ後の一覧
     */
    function nextSelectionAfterFilter(
        previousId: string | null,
        newFiltered: ClipSelectorItem[]
    ): number {
        if (previousId === null) {
            return newFiltered.length;
        }
        const index = newFiltered.findIndex((item) => item.id === previousId);
        return index >= 0 ? index : 0;
    }

    /**
     * 空一覧時のメッセージを返す
     *
     * @returns 空でない場合は `null`
     */
    function emptyListMessage(totalItems: number, filteredLength: number): string | null {
        if (filteredLength > 0) {
            return null;
        }
        return totalItems === 0 ? '登録クリップがありません' : '該当する登録クリップがありません';
    }

    /**
     * 画像ホバープレビューを表示できるかどうか
     */
    function canShowImageHoverPreview(item: ClipSelectorItem): boolean {
        return item.kind === 'image' && !!imagePreviewSource(item);
    }

    /**
     * 選択確定時のアクションを返す
     */
    function resolveActivateSelection(
        selectedIndex: number,
        filtered: ClipSelectorItem[]
    ): { type: 'register' } | { type: 'select'; id: string } | null {
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
     */
    function selectMessage(id: string): string {
        return 'select:' + id;
    }

    /**
     * 登録 IPC メッセージを組み立てる
     */
    function registerMessage(): string {
        return 'register';
    }

    /**
     * 削除 IPC メッセージを組み立てる
     */
    function deleteMessage(id: string): string {
        return 'delete:' + id;
    }

    /**
     * 並べ替え IPC メッセージを組み立てる
     */
    function moveMessage(id: string, direction: 'up' | 'down'): string {
        return 'move:' + id + ':' + direction;
    }

    /**
     * テンプレート切り替え IPC メッセージを組み立てる
     */
    function templateMessage(id: string, enabled: boolean): string {
        return 'template:' + id + ':' + (enabled ? 'on' : 'off');
    }

    /**
     * 選択行のテンプレート展開を切り替えたときの要求内容を返す
     *
     * @returns 登録行・画像・範囲外は `null`
     */
    function resolveTemplateToggle(
        selectedIndex: number,
        filtered: ClipSelectorItem[]
    ): { id: string; enabled: boolean } | null {
        const item = filtered[selectedIndex];
        if (!item || item.kind === 'image') {
            return null;
        }
        return { id: item.id, enabled: !item.template };
    }

    /**
     * 連番リセット IPC メッセージを組み立てる
     */
    function counterResetMessage(id: string): string {
        return 'counter-reset:' + id;
    }

    /**
     * 選択行の連番をリセットするときの要求内容を返す
     *
     * @returns 連番が未使用のクリップ・画像・登録行・範囲外は `null`
     */
    function resolveCounterReset(
        selectedIndex: number,
        filtered: ClipSelectorItem[]
    ): { id: string } | null {
        const item = filtered[selectedIndex];
        if (!item || item.kind === 'image' || !item.counter) {
            return null;
        }
        return { id: item.id };
    }

    /**
     * 連番バッジの表示文字列を返す
     *
     * @returns 連番が未使用のときは空文字
     */
    function counterBadgeText(item: ClipSelectorItem): string {
        return (item.counter ?? 0) > 0 ? '連番 ' + item.counter : '';
    }

    /**
     * 全文取得 IPC メッセージを組み立てる
     */
    function fullTextMessage(id: string): string {
        return 'full:' + id;
    }

    /**
     * 全文のホバー表示を要求できるクリップかどうか
     */
    function canRequestFullText(item: ClipSelectorItem): boolean {
        return item.kind !== 'image';
    }

    /**
     * ホバー表示する全文を求める
     *
     * 一覧のプレビューと同じ内容でも表示する (一覧では改行が潰れ、見切れもあるため)
     *
     * @param text - Rust から受け取った全文
     * @returns 表示しない場合は `null`
     */
    function fullTextToShow(text: unknown): string | null {
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
     * @param query - 正規化済みクエリ
     */
    function canReorder(query: string): boolean {
        return !query;
    }

    /**
     * 選択行を指定方向へ移動したときの移動先インデックスを返す
     *
     * @param query - 正規化済みクエリ
     * @param index - 選択中の行インデックス
     * @returns 移動できない場合 (検索中・登録行・端) は `null`
     */
    function moveTargetIndex(
        query: string,
        index: number,
        filteredLength: number,
        direction: 'up' | 'down'
    ): number | null {
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
     * 削除要求を確認待ちにするか、実際に削除するかを決める
     *
     * 確認待ちの行に対する 2 回目の要求だけを削除として扱う
     *
     * @param pendingId - 確認待ちのクリップ ID
     * @param id - 削除を要求されたクリップ ID
     */
    function resolveDeleteRequest(pendingId: string | null, id: string): 'confirm' | 'delete' {
        return pendingId === id ? 'delete' : 'confirm';
    }

    /**
     * Del キーの扱いを決める
     *
     * 検索欄に文字があるときは検索欄の文字削除に任せる
     * キーリピートは、確認待ちの行をそのまま削除してしまうため無視する
     *
     * @param inputValue - 検索欄の現在値 (正規化前)
     * @param repeat - キーリピートによるイベントか
     */
    function resolveDeleteKey(inputValue: string, repeat: boolean): 'text' | 'ignore' | 'delete' {
        if (inputValue !== '') {
            return 'text';
        }
        return repeat ? 'ignore' : 'delete';
    }

    /**
     * 「クリップボードを登録」行の内部 HTML を組み立てる
     */
    function buildRegisterRowInnerHtml(
        query: string,
        highlightText: (text: string, query: string) => string
    ): string {
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
        selectedItemId,
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
        resolveDeleteRequest,
        resolveDeleteKey,
        buildRegisterRowInnerHtml,
    };
})();

export default ClipSelectorLogic;

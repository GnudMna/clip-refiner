import { escapeHtml } from '../shared/html_escape.ts';

/** 選択状態のコントローラ */
interface SelectionController {
    /** 現在の選択インデックス */
    index: number;
    /** インデックスを行数の範囲に収める */
    clampSelectedIndex: () => void;
    /** 選択行のハイライトを更新 */
    updateSelectionHighlight: () => void;
    /** 選択行を表示範囲内にスクロール */
    scrollSelectedIntoView: () => void;
    /** 選択インデックスを設定 */
    setSelectedIndex: (index: number, scroll: boolean) => void;
    /** 相対移動 */
    moveSelection: (delta: number) => void;
}

/**
 * クイックセレクター / 登録クリップセレクターで共有する UI ユーティリティ
 */
const SelectorCommon = (function () {
    'use strict';

    /**
     * 検索クエリを正規化する (小文字化・前後空白除去)
     */
    function normalizeQuery(query: string): string {
        return String(query || '')
            .toLowerCase()
            .trim();
    }

    /**
     * 小文字化したテキストでクエリを探し、元のテキスト上の範囲を返す
     *
     * `İ` のように小文字化で長さが変わる文字があると、小文字化後の位置は元の位置と一致しない
     * 長さが変わる場合は 1 文字ずつ小文字化して位置を対応づけ、一致が文字の途中に掛かるときは
     * その文字全体を範囲に含める
     *
     * @param text - 対象テキスト
     * @param query - 正規化済み検索クエリ
     * @returns 元テキスト上の範囲。一致しなければ null
     */
    function findMatchRange(text: string, query: string): { index: number; end: number } | null {
        const lowerText = text.toLowerCase();
        if (lowerText.length === text.length) {
            const index = lowerText.indexOf(query);
            return index === -1 ? null : { index, end: index + query.length };
        }

        let lower = '';
        // 小文字化後の各 UTF-16 コードユニットが由来する、元テキスト上の文字の範囲
        const starts = [];
        const ends = [];
        let offset = 0;
        for (const ch of text) {
            const lowered = ch.toLowerCase();
            for (let i = 0; i < lowered.length; i++) {
                starts.push(offset);
                ends.push(offset + ch.length);
            }
            lower += lowered;
            offset += ch.length;
        }

        const index = lower.indexOf(query);
        if (index === -1 || query.length === 0) {
            return null;
        }
        const start = starts[index];
        const end = ends[index + query.length - 1];
        if (start === undefined || end === undefined) {
            return null;
        }
        return { index: start, end };
    }

    /**
     * 検索クエリに一致する部分を `<mark>` でハイライトする
     *
     * @param text - 対象テキスト
     * @param query - 正規化済み検索クエリ
     * @returns エスケープ済み HTML
     */
    function highlightText(text: string, query: string): string {
        const safe = escapeHtml(text);
        if (!query) {
            return safe;
        }

        const range = findMatchRange(String(text), query);
        if (!range) {
            return safe;
        }

        const { index, end } = range;
        return (
            escapeHtml(text.slice(0, index)) +
            '<mark>' +
            escapeHtml(text.slice(index, end)) +
            '</mark>' +
            escapeHtml(text.slice(end))
        );
    }

    /**
     * 選択インデックスを行数の範囲に収める
     */
    function clampIndex(index: number, count: number): number {
        if (count === 0) {
            return 0;
        }
        if (index >= count) {
            return count - 1;
        }
        if (index < 0) {
            return 0;
        }
        return index;
    }

    /**
     * リスト選択状態を管理するコントローラを生成する
     *
     * @param results - 結果リストのコンテナ
     * @param getRowCount - 行数を返す関数
     */
    function createSelection(results: HTMLElement, getRowCount: () => number): SelectionController {
        let selectedIndex = 0;

        /** 選択インデックスを行数の範囲に収める */
        function clampSelectedIndex(): void {
            selectedIndex = clampIndex(selectedIndex, getRowCount());
        }

        /** 選択行の `.selected` クラスを更新する */
        function updateSelectionHighlight(): void {
            results.querySelectorAll('.item').forEach((element, index) => {
                element.classList.toggle('selected', index === selectedIndex);
            });
        }

        /** 選択行を表示範囲内にスクロールする */
        function scrollSelectedIntoView(): void {
            results.querySelectorAll('.item')[selectedIndex]?.scrollIntoView({ block: 'nearest' });
        }

        function setSelectedIndex(index: number, scroll: boolean): void {
            const count = getRowCount();
            if (count === 0) {
                return;
            }

            const next = Math.max(0, Math.min(index, count - 1));
            if (next === selectedIndex) {
                if (scroll) {
                    scrollSelectedIntoView();
                }
                return;
            }

            selectedIndex = next;
            updateSelectionHighlight();
            if (scroll) {
                scrollSelectedIntoView();
            }
        }

        function moveSelection(delta: number): void {
            if (getRowCount() === 0) {
                return;
            }
            setSelectedIndex(selectedIndex + delta, true);
        }

        return {
            get index() {
                return selectedIndex;
            },
            set index(value) {
                selectedIndex = value;
            },
            clampSelectedIndex,
            updateSelectionHighlight,
            scrollSelectedIntoView,
            setSelectedIndex,
            moveSelection,
        };
    }

    /**
     * 矢印キー (↑/↓) の選択移動を処理する
     *
     * @returns 処理した場合は `true`
     */
    function handleArrowKeys(
        event: KeyboardEvent,
        moveSelection: (delta: number) => void
    ): boolean {
        if (event.key === 'ArrowDown') {
            moveSelection(1);
            event.preventDefault();
            return true;
        }
        if (event.key === 'ArrowUp') {
            moveSelection(-1);
            event.preventDefault();
            return true;
        }
        return false;
    }

    /**
     * Ctrl+Home / Ctrl+End の先頭/末尾移動を処理する
     *
     * 修飾キーなしの Home / End は検索欄のカーソル移動に使うため扱わない
     *
     * @returns 処理した場合は `true`
     */
    function handleHomeEndKeys(
        event: KeyboardEvent,
        getRowCount: () => number,
        setSelectedIndex: (index: number, scroll: boolean) => void
    ): boolean {
        if (!(event.ctrlKey || event.metaKey) || event.shiftKey || event.altKey) {
            return false;
        }
        if (event.key === 'Home') {
            if (getRowCount() > 0) {
                setSelectedIndex(0, true);
                event.preventDefault();
            }
            return true;
        }
        if (event.key === 'End') {
            if (getRowCount() > 0) {
                setSelectedIndex(getRowCount() - 1, true);
                event.preventDefault();
            }
            return true;
        }
        return false;
    }

    /**
     * Escape キーを処理する
     *
     * 入力欄に文字がある場合はフィルタをクリアし、空の場合はウィンドウを閉じる
     *
     * @returns 処理した場合は `true`
     */
    function handleEscapeKey(
        event: KeyboardEvent,
        input: HTMLInputElement,
        onClearFilter: () => void,
        onClose: () => void
    ): boolean {
        if (event.key !== 'Escape') {
            return false;
        }
        if (input.value) {
            input.value = '';
            onClearFilter();
            event.preventDefault();
            return true;
        }
        onClose();
        event.preventDefault();
        return true;
    }

    /**
     * 検索入力欄にフォーカスし全選択する
     */
    function focusSearchInput(input: HTMLInputElement): void {
        input.focus();
        input.select();
    }

    /**
     * カテゴリ見出しの内部 HTML を組み立てる
     */
    function buildCategoryHeaderInnerHtml(
        category: string,
        escapeHtmlFn: (text: string) => string
    ): string {
        return (
            '<span class="category-accent" aria-hidden="true"></span>' +
            '<span class="category-label">' +
            escapeHtmlFn(category) +
            '</span>'
        );
    }

    return {
        escapeHtml,
        normalizeQuery,
        highlightText,
        clampIndex,
        createSelection,
        handleArrowKeys,
        handleHomeEndKeys,
        handleEscapeKey,
        focusSearchInput,
        buildCategoryHeaderInnerHtml,
    };
})();

export default SelectorCommon;

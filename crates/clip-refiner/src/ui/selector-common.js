import { escapeHtml } from './html_escape.js';

/**
 * クイックセレクター / 登録クリップセレクターで共有する UI ユーティリティ
 *
 * @namespace SelectorCommon
 */
const SelectorCommon = (function () {
    'use strict';

    /**
     * 検索クエリを正規化する (小文字化・前後空白除去)
     *
     * @param {string} query
     * @returns {string}
     */
    function normalizeQuery(query) {
        return String(query || '')
            .toLowerCase()
            .trim();
    }

    /**
     * 小文字化したテキストでクエリを探し、元のテキスト上の範囲を返す
     *
     * `İ` のように小文字化で長さが変わる文字があると、小文字化後の位置は元の位置と一致しない。
     * 長さが変わる場合は 1 文字ずつ小文字化して位置を対応づけ、一致が文字の途中に掛かるときは
     * その文字全体を範囲に含める。
     *
     * @param {string} text - 対象テキスト
     * @param {string} query - 正規化済み検索クエリ
     * @returns {{index: number, end: number} | null} 元テキスト上の範囲。一致しなければ null
     */
    function findMatchRange(text, query) {
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
        return { index: starts[index], end: ends[index + query.length - 1] };
    }

    /**
     * 検索クエリに一致する部分を `<mark>` でハイライトする
     *
     * @param {string} text - 対象テキスト
     * @param {string} query - 正規化済み検索クエリ
     * @returns {string} エスケープ済み HTML
     */
    function highlightText(text, query) {
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
     *
     * @param {number} index
     * @param {number} count
     * @returns {number}
     */
    function clampIndex(index, count) {
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
     * WebView IPC にメッセージを送信する
     *
     * @param {string} message
     */
    function postIpc(message) {
        window.ipc.postMessage(message);
    }

    /**
     * セレクターウィンドウを閉じる
     */
    function close() {
        postIpc('close');
    }

    /**
     * @typedef {Object} SelectionController
     * @property {number} index - 現在の選択インデックス
     * @property {() => void} clampSelectedIndex - インデックスを行数の範囲に収める
     * @property {() => void} updateSelectionHighlight - 選択行のハイライトを更新
     * @property {() => void} scrollSelectedIntoView - 選択行を表示範囲内にスクロール
     * @property {(index: number, scroll: boolean) => void} setSelectedIndex - 選択インデックスを設定
     * @property {(delta: number) => void} moveSelection - 相対移動
     */

    /**
     * リスト選択状態を管理するコントローラを生成する
     *
     * @param {HTMLElement} results - 結果リストのコンテナ
     * @param {() => number} getRowCount - 行数を返す関数
     * @returns {SelectionController}
     */
    function createSelection(results, getRowCount) {
        let selectedIndex = 0;

        /** 選択インデックスを行数の範囲に収める */
        function clampSelectedIndex() {
            selectedIndex = clampIndex(selectedIndex, getRowCount());
        }

        /** 選択行の `.selected` クラスを更新する */
        function updateSelectionHighlight() {
            results.querySelectorAll('.item').forEach((element, index) => {
                element.classList.toggle('selected', index === selectedIndex);
            });
        }

        /** 選択行を表示範囲内にスクロールする */
        function scrollSelectedIntoView() {
            results.querySelectorAll('.item')[selectedIndex]?.scrollIntoView({ block: 'nearest' });
        }

        /**
         * @param {number} index
         * @param {boolean} scroll
         */
        function setSelectedIndex(index, scroll) {
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

        /**
         * @param {number} delta
         */
        function moveSelection(delta) {
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
     * @param {KeyboardEvent} event
     * @param {(delta: number) => void} moveSelection
     * @returns {boolean} 処理した場合は `true`
     */
    function handleArrowKeys(event, moveSelection) {
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
     * @param {KeyboardEvent} event
     * @param {() => number} getRowCount
     * @param {(index: number, scroll: boolean) => void} setSelectedIndex
     * @returns {boolean} 処理した場合は `true`
     */
    function handleHomeEndKeys(event, getRowCount, setSelectedIndex) {
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
     * @param {KeyboardEvent} event
     * @param {HTMLInputElement} input
     * @param {() => void} onClearFilter
     * @param {() => void} onClose
     * @returns {boolean} 処理した場合は `true`
     */
    function handleEscapeKey(event, input, onClearFilter, onClose) {
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
     *
     * @param {HTMLInputElement} input
     */
    function focusSearchInput(input) {
        input.focus();
        input.select();
    }

    /**
     * カテゴリ見出しの内部 HTML を組み立てる
     *
     * @param {string} category
     * @param {(text: string) => string} escapeHtmlFn
     * @returns {string}
     */
    function buildCategoryHeaderInnerHtml(category, escapeHtmlFn) {
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
        postIpc,
        close,
        createSelection,
        handleArrowKeys,
        handleHomeEndKeys,
        handleEscapeKey,
        focusSearchInput,
        buildCategoryHeaderInnerHtml,
    };
})();

export default SelectorCommon;

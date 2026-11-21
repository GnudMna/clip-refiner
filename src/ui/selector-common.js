// クイックセレクター / 登録クリップセレクターで共有する UI ユーティリティ
(function () {
    'use strict';

    function escapeHtml(text) {
        return String(text)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function normalizeQuery(query) {
        return query.toLowerCase().trim();
    }

    function highlightText(text, query) {
        const safe = escapeHtml(text);
        if (!query) {
            return safe;
        }

        const lowerText = String(text).toLowerCase();
        const index = lowerText.indexOf(query);
        if (index === -1) {
            return safe;
        }

        const end = index + query.length;
        return (
            escapeHtml(text.slice(0, index)) +
            '<mark>' + escapeHtml(text.slice(index, end)) + '</mark>' +
            escapeHtml(text.slice(end))
        );
    }

    function postIpc(message) {
        window.ipc.postMessage(message);
    }

    function close() {
        postIpc('close');
    }

    function createSelection(results, getRowCount) {
        let selectedIndex = 0;

        function clampSelectedIndex() {
            const count = getRowCount();
            if (count === 0) {
                selectedIndex = 0;
                return;
            }
            if (selectedIndex >= count) {
                selectedIndex = count - 1;
            }
            if (selectedIndex < 0) {
                selectedIndex = 0;
            }
        }

        function updateSelectionHighlight() {
            results.querySelectorAll('.item').forEach((element, index) => {
                element.classList.toggle('selected', index === selectedIndex);
            });
        }

        function scrollSelectedIntoView() {
            results.querySelectorAll('.item')[selectedIndex]?.scrollIntoView({ block: 'nearest' });
        }

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

    function handleHomeEndKeys(event, getRowCount, setSelectedIndex) {
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

    function focusSearchInput(input) {
        input.focus();
        input.select();
    }

    window.SelectorCommon = {
        escapeHtml,
        normalizeQuery,
        highlightText,
        postIpc,
        close,
        createSelection,
        handleArrowKeys,
        handleHomeEndKeys,
        handleEscapeKey,
        focusSearchInput,
    };
})();

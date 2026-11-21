/**
 * クイックセレクター UI (DOM / IPC)
 */
import SelectorCommon from './selector-common.js';
import QuickSelectorLogic from './quick_selector_logic.js';

const {
    normalizeQuery,
    highlightText,
    escapeHtml,
    postIpc,
    close: closeSelector,
    createSelection,
    handleArrowKeys,
    handleHomeEndKeys,
    handleEscapeKey,
    focusSearchInput,
    buildCategoryHeaderInnerHtml,
} = SelectorCommon;

const {
    FAVORITES_CATEGORY,
    favoriteIds,
    buildDisplayList,
    sectionCategory,
    nextIndexAfterFilter,
    shouldShowReorderButtons,
    canMoveSelectedFavorite,
    buildItemInnerHtml,
    selectMessage,
    toggleFavoriteMessage,
    moveFavoriteMessage,
} = QuickSelectorLogic;

/** @type {import('./quick_selector_logic').QuickSelectorMode[]} */
let modes = JSON.parse(document.getElementById('modes-data').textContent);
/** @type {HTMLInputElement} */
const input = document.getElementById('input');
/** @type {HTMLElement} */
const results = document.getElementById('results');

/** @type {string|null} */
let currentModeId = null;
/** @type {import('./quick_selector_logic').QuickSelectorMode[]} */
let filtered = buildDisplayList(modes, '');
const selection = createSelection(results, () => filtered.length);

/**
 * モード一覧を差し替える
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode[]} nextModes
 */
function loadModes(nextModes) {
    modes = nextModes;
    filtered = buildDisplayList(modes, '');
}

/**
 * お気に入り行用の並べ替えボタンを生成する
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode} mode
 * @param {string} query
 * @returns {HTMLDivElement|null}
 */
function createReorderButtons(mode, query) {
    if (!shouldShowReorderButtons(mode, query)) {
        return null;
    }

    const ids = favoriteIds(modes);
    const index = ids.indexOf(mode.id);
    if (index === -1) {
        return null;
    }

    const container = document.createElement('div');
    container.className = 'reorder-buttons';

    const upButton = document.createElement('button');
    upButton.type = 'button';
    upButton.className = 'reorder-button';
    upButton.textContent = '▲';
    upButton.title = '上へ移動';
    upButton.disabled = index === 0;
    upButton.onclick = (event) => {
        event.stopPropagation();
        moveFavorite(mode.id, 'up');
    };

    const downButton = document.createElement('button');
    downButton.type = 'button';
    downButton.className = 'reorder-button';
    downButton.textContent = '▼';
    downButton.title = '下へ移動';
    downButton.disabled = index === ids.length - 1;
    downButton.onclick = (event) => {
        event.stopPropagation();
        moveFavorite(mode.id, 'down');
    };

    container.appendChild(upButton);
    container.appendChild(downButton);
    return container;
}

/**
 * お気に入りトグルボタンを生成する
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode} mode
 * @returns {HTMLButtonElement}
 */
function createFavoriteButton(mode) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'favorite-button' + (mode.favorite ? ' is-favorite' : '');
    button.title = mode.favorite ? 'お気に入りから解除' : 'お気に入りに登録';
    button.setAttribute('aria-label', button.title);
    button.textContent = mode.favorite ? '★' : '☆';
    button.onclick = (event) => {
        event.stopPropagation();
        toggleFavorite(mode.id);
    };
    return button;
}

/**
 * モード 1 行分の DOM 要素を生成する
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode} mode
 * @param {number} index
 * @param {string} query
 * @returns {HTMLDivElement}
 */
function createItem(mode, index, query) {
    const isCurrent = mode.id === currentModeId;
    const div = document.createElement('div');
    div.className =
        'item' + (index === selection.index ? ' selected' : '') + (isCurrent ? ' current' : '');

    div.innerHTML = buildItemInnerHtml(mode, query, isCurrent, highlightText);

    const reorderButtons = createReorderButtons(mode, query);
    if (reorderButtons) {
        div.appendChild(reorderButtons);
    }
    div.appendChild(createFavoriteButton(mode));

    div.onmouseenter = () => selection.setSelectedIndex(index, false);
    div.onclick = () => select(mode.id);
    return div;
}

/**
 * 結果リストを再描画する
 *
 * @param {boolean} scrollToSelection
 */
function renderList(scrollToSelection) {
    results.innerHTML = '';
    const query = normalizeQuery(input.value);
    selection.clampSelectedIndex();

    if (filtered.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'empty';
        empty.textContent = '該当するモードがありません';
        results.appendChild(empty);
        return;
    }

    let itemIndex = 0;
    let lastCategory = null;
    let currentSection = null;
    let rootSection = null;

    filtered.forEach((mode) => {
        let parent = results;
        const category = sectionCategory(mode, query);

        if (!query) {
            if (category) {
                if (category !== lastCategory) {
                    currentSection = document.createElement('div');
                    currentSection.className = 'category-section';
                    if (category === FAVORITES_CATEGORY) {
                        currentSection.classList.add('category-section--favorites');
                    }

                    const header = document.createElement('div');
                    header.className = 'category-header';
                    header.innerHTML = buildCategoryHeaderInnerHtml(category, escapeHtml);
                    currentSection.appendChild(header);
                    results.appendChild(currentSection);
                    lastCategory = category;
                }
                parent = currentSection;
            } else {
                if (!rootSection) {
                    rootSection = document.createElement('div');
                    rootSection.className = 'category-section category-section--plain';
                    results.appendChild(rootSection);
                }
                parent = rootSection;
            }
        }

        parent.appendChild(createItem(mode, itemIndex, query));
        itemIndex++;
    });

    if (scrollToSelection) {
        selection.scrollSelectedIntoView();
    }
}

/** 検索フィルタを適用して一覧を再描画する */
function filter() {
    const query = normalizeQuery(input.value);
    const previousId = filtered[selection.index]?.id;

    filtered = buildDisplayList(modes, query);
    selection.index = nextIndexAfterFilter(filtered, previousId);
    renderList(true);
}

/**
 * モードを選択して IPC 送信する
 *
 * @param {string} id
 */
function select(id) {
    postIpc(selectMessage(id));
}

/**
 * お気に入りの ON/OFF を IPC 送信する
 *
 * @param {string} id
 */
function toggleFavorite(id) {
    postIpc(toggleFavoriteMessage(id));
}

/**
 * お気に入りの並べ替えを IPC 送信する
 *
 * @param {string} id
 * @param {'up'|'down'} direction
 */
function moveFavorite(id, direction) {
    postIpc(moveFavoriteMessage(id, direction));
}

/**
 * 選択中のお気に入りを上下に移動する
 *
 * @param {number} delta - 負数で上、正数で下
 * @returns {boolean} 移動した場合は `true`
 */
function moveSelectedFavorite(delta) {
    const mode = filtered[selection.index];
    if (!canMoveSelectedFavorite(normalizeQuery(input.value), mode)) {
        return false;
    }

    moveFavorite(mode.id, delta < 0 ? 'up' : 'down');
    return true;
}

/**
 * 現在モードにフォーカスして一覧を再描画する
 *
 * @param {string|null} modeId
 */
function applyFocus(modeId) {
    currentModeId = modeId;
    input.value = '';
    filter();

    const index = filtered.findIndex((mode) => mode.id === currentModeId);
    if (index !== -1) {
        selection.index = index;
    }

    renderList(true);
    focusSearchInput(input);
}

input.oninput = filter;

/**
 * キーボード入力を処理する
 *
 * Rust WebView から `window.onkeydown(event)` として呼び出す
 *
 * @param {KeyboardEvent} e
 */
window.onkeydown = (e) => {
    if (handleArrowKeys(e, selection.moveSelection)) {
        return;
    }
    if (handleHomeEndKeys(e, () => filtered.length, selection.setSelectedIndex)) {
        return;
    }
    if (e.key === 'Enter') {
        if (filtered[selection.index]) {
            select(filtered[selection.index].id);
            e.preventDefault();
        }
        return;
    }
    if (handleEscapeKey(e, input, filter, closeSelector)) {
        return;
    }
    if (e.key.toLowerCase() === 'd' && (e.ctrlKey || e.metaKey)) {
        if (filtered[selection.index]) {
            toggleFavorite(filtered[selection.index].id);
            e.preventDefault();
        }
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowUp') {
        if (moveSelectedFavorite(-1)) {
            e.preventDefault();
        }
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowDown') {
        if (moveSelectedFavorite(1)) {
            e.preventDefault();
        }
    }
};

/**
 * UI 表示時にフォーカスを当てる
 *
 * @param {string|null} modeId
 * @param {import('./quick_selector_logic').QuickSelectorMode[]} [modesJson]
 */
window.focusInput = (modeId, modesJson) => {
    if (modesJson) {
        loadModes(modesJson);
    }
    applyFocus(modeId);
};

/**
 * お気に入り更新後に一覧を再描画する
 *
 * @param {string|null} modeId
 * @param {import('./quick_selector_logic').QuickSelectorMode[]} [modesJson]
 */
window.refreshModes = (modeId, modesJson) => {
    const previousId = filtered[selection.index]?.id;
    if (modesJson) {
        loadModes(modesJson);
    }
    currentModeId = modeId;
    filtered = buildDisplayList(modes, normalizeQuery(input.value));
    selection.index = nextIndexAfterFilter(filtered, previousId);
    renderList(true);
};

renderList(false);

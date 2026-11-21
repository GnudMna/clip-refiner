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
    isCategoryRow,
    categoryRowIndexOf,
    withCategoryExpanded,
    toExpandedCategories,
    expandedCategoriesMessage,
    favoriteIds,
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
} = QuickSelectorLogic;

/** @type {import('./quick_selector_logic').QuickSelectorMode[]} */
let modes = JSON.parse(document.getElementById('modes-data').textContent);
/** @type {HTMLInputElement} */
const input = document.getElementById('input');
/** @type {HTMLElement} */
const results = document.getElementById('results');

/**
 * 展開中カテゴリをアプリ側へ通知する
 *
 * `WebView` を破棄しても次の表示で `focusInput` から復元できるよう、アプリ起動中はアプリ側で保持する
 */
function saveExpandedCategories() {
    postIpc(expandedCategoriesMessage(expandedCategories));
}

/** @type {string|null} */
let currentModeId = null;
/** @type {Set<string>} */
let expandedCategories = new Set();
/** @type {import('./quick_selector_logic').QuickSelectorMode[]} */
let filtered = buildDisplayList(modes, '', expandedCategories);
const selection = createSelection(results, () => filtered.length);

/**
 * モード一覧を差し替える
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode[]} nextModes
 */
function loadModes(nextModes) {
    modes = nextModes;
    filtered = buildDisplayList(modes, '', expandedCategories);
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
 * カテゴリ見出し行 (開閉トグル) の DOM 要素を生成する
 *
 * @param {import('./quick_selector_logic').QuickSelectorMode} row
 * @param {number} index
 * @returns {HTMLDivElement}
 */
function createCategoryRow(row, index) {
    const div = document.createElement('div');
    div.className =
        'item category-header category-toggle' +
        (row.expanded ? ' is-expanded' : '') +
        (index === selection.index ? ' selected' : '');
    div.setAttribute('role', 'button');
    div.setAttribute('aria-expanded', row.expanded ? 'true' : 'false');
    div.innerHTML = buildCategoryHeaderInnerHtml(row.category, escapeHtml);

    const chevron = document.createElement('span');
    chevron.className = 'category-chevron';
    chevron.setAttribute('aria-hidden', 'true');
    chevron.textContent = row.expanded ? '▾' : '▸';
    div.prepend(chevron);

    const count = document.createElement('span');
    count.className = 'category-count';
    count.textContent = String(row.count);
    div.appendChild(count);

    // mouseenter は再描画後に止まったカーソル下でも発火し、キー操作の選択を奪うため mousemove で追う
    div.onmousemove = () => selection.setSelectedIndex(index, false);
    div.onclick = () => {
        selection.index = index;
        setCategoryExpanded(row.category, !row.expanded);
    };
    return div;
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

    // mouseenter は再描画後に止まったカーソル下でも発火し、キー操作の選択を奪うため mousemove で追う
    div.onmousemove = () => selection.setSelectedIndex(index, false);
    div.onclick = (e) => select(mode.id, e.ctrlKey || e.metaKey);
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

        if (isCategoryRow(mode)) {
            currentSection = document.createElement('div');
            currentSection.className = 'category-section category-section--collapsible';
            currentSection.appendChild(createCategoryRow(mode, itemIndex));
            results.appendChild(currentSection);
            lastCategory = category;
            itemIndex++;
            return;
        }

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

    filtered = buildDisplayList(modes, query, expandedCategories);
    selection.index = nextIndexAfterFilter(filtered, previousId);
    renderList(true);
}

/**
 * カテゴリを展開 / 折りたたみ、選択中の行を維持して再描画する
 *
 * @param {string} category
 * @param {boolean} expand
 */
function setCategoryExpanded(category, expand) {
    expandedCategories = withCategoryExpanded(expandedCategories, category, expand);
    saveExpandedCategories();
    filter();
}

/**
 * ←/→ キーでカテゴリを開閉する
 *
 * 見出し行では → で展開・← で折りたたむ。カテゴリ内のモード行で ← を押すと見出しへ戻って折りたたむ
 *
 * @param {KeyboardEvent} e
 * @returns {boolean} 処理した場合は `true`
 */
function handleCategoryKeys(e) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') {
        return false;
    }
    if (normalizeQuery(input.value) || e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) {
        return false;
    }

    const headerIndex = categoryRowIndexOf(filtered, selection.index);
    if (headerIndex === -1) {
        return false;
    }
    const header = filtered[headerIndex];
    const expand = e.key === 'ArrowRight';
    e.preventDefault();
    if (header.expanded === expand) {
        return true;
    }
    if (!expand) {
        selection.index = headerIndex;
    }
    setCategoryExpanded(header.category, expand);
    return true;
}

/**
 * モードを IPC 送信する
 *
 * @param {string} id
 * @param {boolean} setAsAuto - `true` で自動変換モードに設定し、`false` で一回だけ適用する
 */
function select(id, setAsAuto) {
    postIpc(setAsAuto ? selectMessage(id) : applyMessage(id));
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
    if (isCategoryRow(mode)) {
        return false;
    }
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

    let index = filtered.findIndex((mode) => mode.id === currentModeId);
    // 自動変換中のモードが折りたたまれたカテゴリにある場合は開いて選択する
    const current = currentModeId ? modes.find((mode) => mode.id === currentModeId) : null;
    if (index === -1 && current?.category) {
        expandedCategories = withCategoryExpanded(expandedCategories, current.category, true);
        saveExpandedCategories();
        filtered = buildDisplayList(modes, '', expandedCategories);
        index = filtered.findIndex((mode) => mode.id === currentModeId);
    }
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
    // 並べ替えの Ctrl+Shift+↑↓ は選択移動より先に判定する
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowUp') {
        if (moveSelectedFavorite(-1)) {
            e.preventDefault();
        }
        return;
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowDown') {
        if (moveSelectedFavorite(1)) {
            e.preventDefault();
        }
        return;
    }
    if (handleArrowKeys(e, selection.moveSelection)) {
        return;
    }
    if (handleHomeEndKeys(e, () => filtered.length, selection.setSelectedIndex)) {
        return;
    }
    if (handleCategoryKeys(e)) {
        return;
    }
    if (e.key === 'Enter') {
        const row = filtered[selection.index];
        if (isCategoryRow(row)) {
            setCategoryExpanded(row.category, !row.expanded);
            e.preventDefault();
        } else if (row) {
            select(row.id, e.ctrlKey || e.metaKey);
            e.preventDefault();
        }
        return;
    }
    if (handleEscapeKey(e, input, filter, closeSelector)) {
        return;
    }
    if (e.key.toLowerCase() === 'd' && (e.ctrlKey || e.metaKey)) {
        if (filtered[selection.index] && !isCategoryRow(filtered[selection.index])) {
            toggleFavorite(filtered[selection.index].id);
            e.preventDefault();
        }
    }
};

/**
 * UI 表示時にフォーカスを当てる
 *
 * @param {string|null} modeId
 * @param {import('./quick_selector_logic').QuickSelectorMode[]} [modesJson]
 * @param {string[]} [expanded] - アプリ側で保持している展開中カテゴリ
 */
window.focusInput = (modeId, modesJson, expanded) => {
    if (expanded) {
        expandedCategories = toExpandedCategories(expanded);
    }
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
    filtered = buildDisplayList(modes, normalizeQuery(input.value), expandedCategories);
    selection.index = nextIndexAfterFilter(filtered, previousId);
    renderList(true);
};

renderList(false);

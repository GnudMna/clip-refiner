/**
 * 登録クリップセレクター UI (DOM / IPC)
 */
import { requiredById, requiredChild } from './dom.js';

import SelectorCommon from './selector-common.js';
import ClipSelectorLogic from './clip_selector_logic.js';

const {
    normalizeQuery,
    highlightText,
    postIpc,
    close: closeSelectorWindow,
    createSelection,
    handleArrowKeys,
    handleHomeEndKeys,
    handleEscapeKey,
    focusSearchInput,
} = SelectorCommon;

const {
    imagePreviewSource,
    itemMatches,
    isRegisterIndex,
    totalRows,
    nextSelectionAfterFilter,
    emptyListMessage,
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
} = ClipSelectorLogic;

const input = /** @type {HTMLInputElement} */ (document.getElementById('input'));
const results = requiredById('results');

/** @type {Array<{ id: string, label: string, preview?: string, kind: string, template?: boolean, counter?: number, thumbnail?: string, hover_preview?: string }>} */
let items = [];
/** @type {typeof items} */
let filtered = [];
const selection = createSelection(results, () => totalRows(filtered.length));

const imageHoverPreview = requiredById('image-hover-preview');
const imageHoverPreviewImg = /** @type {HTMLImageElement} */ (
    requiredChild(imageHoverPreview, 'img')
);
const imageHoverPreviewCaption = requiredChild(imageHoverPreview, '.image-hover-preview-caption');

/** 画像ホバープレビューを非表示にする */
function hideImageHoverPreview() {
    imageHoverPreview.classList.remove('is-visible');
    imageHoverPreview.setAttribute('aria-hidden', 'true');
    imageHoverPreviewImg.removeAttribute('src');
    imageHoverPreviewCaption.textContent = '';
}

/** 画像ホバープレビューを画面中央付近に配置する */
function positionImageHoverPreview() {
    const margin = 10;

    imageHoverPreview.style.left = '0px';
    imageHoverPreview.style.top = '0px';
    imageHoverPreview.classList.add('is-visible');

    const previewRect = imageHoverPreview.getBoundingClientRect();
    let left = (window.innerWidth - previewRect.width) / 2;
    let top = (window.innerHeight - previewRect.height) / 2;

    left = Math.max(margin, Math.min(left, window.innerWidth - previewRect.width - margin));
    top = Math.max(margin, Math.min(top, window.innerHeight - previewRect.height - margin));

    imageHoverPreview.style.left = `${left}px`;
    imageHoverPreview.style.top = `${top}px`;
}

/**
 * 画像クリップのホバープレビューを表示する
 *
 * @param {{ kind: string, preview?: string, label: string, hover_preview?: string, thumbnail?: string }} item
 * @param {HTMLElement|null} anchorEl
 */
function showImageHoverPreview(item, anchorEl) {
    const src = imagePreviewSource(item);
    if (!src || item.kind !== 'image' || !anchorEl) {
        hideImageHoverPreview();
        return;
    }

    imageHoverPreviewImg.src = src;
    imageHoverPreviewCaption.textContent = item.preview || item.label;
    imageHoverPreview.setAttribute('aria-hidden', 'false');
    positionImageHoverPreview();
}

/**
 * サムネイル要素にホバープレビューをバインドする
 *
 * @param {typeof items[number]} item
 * @param {HTMLElement} previewTarget
 */
function bindImageHoverPreview(item, previewTarget) {
    previewTarget.onmouseenter = () => {
        showImageHoverPreview(item, previewTarget);
    };
    previewTarget.onmouseleave = () => {
        hideImageHoverPreview();
    };
}

results.addEventListener('mouseleave', hideImageHoverPreview);
results.addEventListener('scroll', hideImageHoverPreview, { passive: true });
input.addEventListener('mouseenter', hideImageHoverPreview);

// ======================================================================
// テキスト全文のホバー表示
// ======================================================================
/** 全文を要求するまでのカーソル静止時間 (ms) */
const TEXT_HOVER_DELAY_MS = 400;

const textHoverPreview = requiredById('text-hover-preview');

/** @type {ReturnType<typeof setTimeout>|null} */
let textHoverTimer = null;
/** @type {typeof items[number]|null} */
let textHoverItem = null;
/** @type {HTMLElement|null} */
let textHoverAnchor = null;

/** テキスト全文のホバー表示を非表示にし、待機中の要求を取り消す */
function hideTextHoverPreview() {
    if (textHoverTimer !== null) {
        clearTimeout(textHoverTimer);
        textHoverTimer = null;
    }
    textHoverItem = null;
    textHoverAnchor = null;
    textHoverPreview.classList.remove('is-visible');
    textHoverPreview.setAttribute('aria-hidden', 'true');
    textHoverPreview.textContent = '';
}

/**
 * テキスト全文のホバー表示を対象行の上下どちらか広い側に配置する
 *
 * @param {HTMLElement} anchorEl
 */
function positionTextHoverPreview(anchorEl) {
    const margin = 10;
    const gap = 4;
    const anchorRect = anchorEl.getBoundingClientRect();
    const previewRect = textHoverPreview.getBoundingClientRect();
    const spaceBelow = window.innerHeight - anchorRect.bottom - margin;
    const spaceAbove = anchorRect.top - margin;

    let top =
        previewRect.height + gap <= spaceBelow || spaceBelow >= spaceAbove
            ? anchorRect.bottom + gap
            : anchorRect.top - previewRect.height - gap;
    top = Math.max(margin, Math.min(top, window.innerHeight - previewRect.height - margin));

    textHoverPreview.style.top = `${top}px`;
}

/**
 * カーソルが止まったテキスト行の全文を Rust へ要求する予約をする
 *
 * @param {typeof items[number]} item
 * @param {HTMLElement} rowEl
 */
function scheduleTextHoverPreview(item, rowEl) {
    if (textHoverItem === item) {
        return;
    }
    hideTextHoverPreview();
    if (!canRequestFullText(item)) {
        return;
    }
    textHoverItem = item;
    textHoverAnchor = rowEl;
    textHoverTimer = setTimeout(() => {
        textHoverTimer = null;
        postIpc(fullTextMessage(item.id));
    }, TEXT_HOVER_DELAY_MS);
}

/**
 * Rust から受け取った全文をホバー表示する
 *
 * Rust WebView から `window.showClipFullText(id, text)` として呼び出す
 * 応答待ちの間にカーソルが別の行へ移っていた場合は破棄する
 *
 * @param {number|string} id
 * @param {unknown} text
 */
window.showClipFullText = (id, text) => {
    if (!textHoverItem || !textHoverAnchor || textHoverItem.id !== String(id)) {
        return;
    }
    const body = fullTextToShow(text);
    if (body === null) {
        return;
    }
    textHoverPreview.textContent = body;
    textHoverPreview.setAttribute('aria-hidden', 'false');
    textHoverPreview.classList.add('is-visible');
    positionTextHoverPreview(textHoverAnchor);
};

results.addEventListener('mouseleave', hideTextHoverPreview);
results.addEventListener('scroll', hideTextHoverPreview, { passive: true });
input.addEventListener('mouseenter', hideTextHoverPreview);

/**
 * クリップ 1 行分の DOM 要素を生成する
 *
 * @param {typeof items[number]} item
 * @param {number} index
 * @param {string} query
 * @returns {HTMLDivElement}
 */
function createItem(item, index, query) {
    const div = document.createElement('div');
    const isImage = item.kind === 'image';
    div.className =
        'item' + (isImage ? ' item--image' : '') + (index === selection.index ? ' selected' : '');

    const content = document.createElement('div');
    content.className = 'item-content';

    if (isImage) {
        let previewTarget = null;
        if (item.thumbnail) {
            const img = document.createElement('img');
            img.className = 'item-thumbnail';
            img.src = item.thumbnail;
            img.alt = '';
            img.loading = 'lazy';
            content.appendChild(img);
            previewTarget = img;
        } else {
            const placeholder = document.createElement('div');
            placeholder.className = 'item-thumbnail item-thumbnail--placeholder';
            placeholder.textContent = '🖼';
            placeholder.setAttribute('aria-hidden', 'true');
            content.appendChild(placeholder);
            if (imagePreviewSource(item)) {
                previewTarget = placeholder;
            }
        }
        if (previewTarget) {
            bindImageHoverPreview(item, previewTarget);
        }
    }

    const modeInfo = document.createElement('div');
    modeInfo.className = 'mode-info';
    const label = document.createElement('span');
    label.className = 'label';
    label.innerHTML = highlightText(item.label, query);
    modeInfo.appendChild(label);
    if (item.template) {
        const badge = document.createElement('span');
        badge.className = 'template-badge';
        badge.textContent = 'テンプレート';
        badge.title = 'コピー時に {date} などを展開する';
        modeInfo.appendChild(badge);
    }
    const counterText = counterBadgeText(item);
    if (counterText) {
        const counterBadge = document.createElement('span');
        counterBadge.className = 'template-badge';
        counterBadge.textContent = counterText;
        counterBadge.title = '{counter} で最後に使った連番。Ctrl+R で 1 から数え直す';
        modeInfo.appendChild(counterBadge);
    }
    content.appendChild(modeInfo);

    if (item.preview && !isImage) {
        const hint = document.createElement('span');
        hint.className = 'value-hint';
        hint.innerHTML = highlightText(item.preview, query);
        content.appendChild(hint);
    }

    div.appendChild(content);

    if (canReorder(query)) {
        div.appendChild(createReorderButtons(index));
    }

    const deleteBtn = document.createElement('button');
    deleteBtn.type = 'button';
    deleteBtn.className = 'item-delete';
    deleteBtn.title = '削除';
    deleteBtn.setAttribute('aria-label', '削除');
    deleteBtn.textContent = '×';
    deleteBtn.onclick = (e) => {
        e.stopPropagation();
        deleteItem(item.id);
    };
    div.appendChild(deleteBtn);

    // mouseenter は再描画後に止まったカーソル下でも発火し、キー操作の選択を奪うため mousemove で追う
    div.onmousemove = () => {
        selection.setSelectedIndex(index, false);
        scheduleTextHoverPreview(item, div);
    };
    div.onmouseleave = hideTextHoverPreview;
    div.onclick = () => select(item.id);

    return div;
}

/**
 * 並べ替えボタン (▲▼) を生成する
 *
 * @param {number} index
 * @returns {HTMLDivElement}
 */
function createReorderButtons(index) {
    const container = document.createElement('div');
    container.className = 'reorder-buttons';

    /** @type {Array<['up'|'down', string, string]>} */
    const buttons = [
        ['up', '▲', '上へ移動'],
        ['down', '▼', '下へ移動'],
    ];
    for (const [direction, text, title] of buttons) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'reorder-button';
        button.textContent = text;
        button.title = title;
        button.setAttribute('aria-label', title);
        button.disabled = moveTargetIndex('', index, filtered.length, direction) === null;
        button.onclick = (event) => {
            event.stopPropagation();
            moveAt(index, direction);
        };
        container.appendChild(button);
    }
    return container;
}

/**
 * 「クリップボードを登録」行の DOM 要素を生成する
 *
 * @param {number} index
 * @param {string} query
 * @returns {HTMLDivElement}
 */
function createRegisterRow(index, query) {
    const div = document.createElement('div');
    div.className = 'item action' + (index === selection.index ? ' selected' : '');

    div.innerHTML = buildRegisterRowInnerHtml(query, highlightText);

    // mouseenter は再描画後に止まったカーソル下でも発火し、キー操作の選択を奪うため mousemove で追う
    div.onmousemove = () => selection.setSelectedIndex(index, false);
    div.onclick = () => register();
    return div;
}

/**
 * 結果リストを再描画する
 *
 * @param {boolean} scrollToSelection
 */
function renderList(scrollToSelection) {
    hideImageHoverPreview();
    hideTextHoverPreview();
    results.innerHTML = '';
    const query = normalizeQuery(input.value);
    selection.clampSelectedIndex();

    const emptyMessage = emptyListMessage(items.length, filtered.length);
    if (emptyMessage) {
        const empty = document.createElement('div');
        empty.className = 'empty';
        empty.textContent = emptyMessage;
        results.appendChild(empty);
    } else {
        filtered.forEach((item, index) => {
            results.appendChild(createItem(item, index, query));
        });
    }

    results.appendChild(createRegisterRow(filtered.length, query));

    if (scrollToSelection) {
        selection.scrollSelectedIntoView();
    }
}

/** 検索フィルタを適用して一覧を再描画する */
function filter() {
    const query = normalizeQuery(input.value);
    const previousIndex = selection.index;
    const oldFilteredLength = filtered.length;

    filtered = items.filter((item) => itemMatches(item, query));
    selection.index = nextSelectionAfterFilter(previousIndex, oldFilteredLength, filtered.length);
    renderList(true);
}

/**
 * 登録クリップを選択して IPC 送信する
 *
 * @param {string} id
 */
function select(id) {
    postIpc(selectMessage(id));
}

/** クリップボードから新規登録を IPC 送信する */
function register() {
    postIpc(registerMessage());
}

/**
 * 登録クリップを削除する
 *
 * @param {string} id
 */
function deleteItem(id) {
    postIpc(deleteMessage(id));
}

/**
 * 指定行の登録クリップを並べ替える
 *
 * 一覧の再描画後も移動した項目を選択したままにするため、先に選択位置を移動先へ合わせる
 *
 * @param {number} index
 * @param {'up'|'down'} direction
 * @returns {boolean} 移動を要求した場合は `true`
 */
function moveAt(index, direction) {
    const target = moveTargetIndex(normalizeQuery(input.value), index, filtered.length, direction);
    if (target === null) {
        return false;
    }
    selection.index = target;
    postIpc(moveMessage(filtered[index].id, direction));
    return true;
}

/**
 * 選択中のクリップのテンプレート展開を切り替える
 *
 * @returns {boolean} 切り替えを要求した場合は `true`
 */
function toggleSelectedTemplate() {
    const target = resolveTemplateToggle(selection.index, filtered);
    if (!target) {
        return false;
    }
    postIpc(templateMessage(target.id, target.enabled));
    return true;
}

/**
 * 選択中のクリップの連番を最初に戻す
 *
 * @returns {boolean} リセットを要求した場合は `true`
 */
function resetSelectedCounter() {
    const target = resolveCounterReset(selection.index, filtered);
    if (!target) {
        return false;
    }
    postIpc(counterResetMessage(target.id));
    return true;
}

/** 選択中のクリップを削除する */
function deleteSelected() {
    const item = filtered[selection.index];
    if (!item) {
        return;
    }
    deleteItem(item.id);
}

/** セレクターを閉じる */
function closeSelector() {
    hideImageHoverPreview();
    hideTextHoverPreview();
    closeSelectorWindow();
}

/** Enter キー相当の選択確定処理 */
function activateSelection() {
    const action = resolveActivateSelection(selection.index, filtered);
    if (!action) {
        return;
    }
    if (action.type === 'register') {
        register();
        return;
    }
    select(action.id);
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
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
        if (moveAt(selection.index, e.key === 'ArrowUp' ? 'up' : 'down')) {
            e.preventDefault();
        }
        return;
    }
    if (handleArrowKeys(e, selection.moveSelection)) {
        return;
    }
    if (handleHomeEndKeys(e, () => totalRows(filtered.length), selection.setSelectedIndex)) {
        return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 't') {
        toggleSelectedTemplate();
        e.preventDefault();
        return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'r') {
        resetSelectedCounter();
        e.preventDefault();
        return;
    }
    if (e.key === 'Enter' && e.ctrlKey) {
        register();
        e.preventDefault();
        return;
    }
    if (e.key === 'Enter') {
        activateSelection();
        e.preventDefault();
        return;
    }
    if (
        e.key === 'Delete' &&
        !isRegisterIndex(selection.index, filtered.length) &&
        filtered[selection.index]
    ) {
        deleteSelected();
        e.preventDefault();
        return;
    }
    handleEscapeKey(e, input, filter, closeSelector);
};

/**
 * UI 表示時にフォーカスを当てる
 *
 * @param {typeof items} nextItems
 */
window.focusInput = (nextItems) => {
    items = Array.isArray(nextItems) ? nextItems : [];
    input.value = '';
    selection.index = 0;
    filter();
    focusSearchInput(input);
};

/**
 * 登録後などに一覧だけ更新する
 *
 * @param {typeof items} nextItems
 */
window.refreshItems = (nextItems) => {
    items = Array.isArray(nextItems) ? nextItems : [];
    filter();
};

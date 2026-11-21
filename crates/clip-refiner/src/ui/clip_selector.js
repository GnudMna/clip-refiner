/**
 * 登録クリップセレクター UI ロジック
 */
const {
    normalizeQuery,
    highlightText,
    postIpc,
    close,
    createSelection,
    handleArrowKeys,
    handleHomeEndKeys,
    handleEscapeKey,
    focusSearchInput,
} = window.SelectorCommon;

/** @type {HTMLInputElement} */
const input = document.getElementById('input');
/** @type {HTMLElement} */
const results = document.getElementById('results');

/** @type {Array<{ id: string, label: string, preview?: string, kind: string, thumbnail?: string, hover_preview?: string }>} */
let items = [];
/** @type {typeof items} */
let filtered = [];
const selection = createSelection(results, totalRows);

const imageHoverPreview = document.getElementById('image-hover-preview');
const imageHoverPreviewImg = imageHoverPreview.querySelector('img');
const imageHoverPreviewCaption = imageHoverPreview.querySelector('.image-hover-preview-caption');

/**
 * 画像プレビュー用の URL を取得する
 *
 * @param {{ hover_preview?: string, thumbnail?: string }} item
 * @returns {string|null}
 */
function imagePreviewSource(item) {
    return item.hover_preview || item.thumbnail || null;
}

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
 * @param {{ kind: string, preview?: string, label: string }} item
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

/**
 * 検索クエリにクリップが一致するかどうか
 *
 * @param {typeof items[number]} item
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
 * @returns {boolean}
 */
function isRegisterIndex(index) {
    return index === filtered.length;
}

/** @returns {number} 一覧行数 (登録行を含む) */
function totalRows() {
    return filtered.length + 1;
}

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
    div.className = 'item' + (isImage ? ' item--image' : '') + (index === selection.index ? ' selected' : '');

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
    content.appendChild(modeInfo);

    if (item.preview && !isImage) {
        const hint = document.createElement('span');
        hint.className = 'value-hint';
        hint.innerHTML = highlightText(item.preview, query);
        content.appendChild(hint);
    }

    div.appendChild(content);

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

    div.onmouseenter = () => selection.setSelectedIndex(index, false);
    div.onclick = () => select(item.id);

    return div;
}

/**
 * 「クリップボードを登録」行の DOM 要素を生成する
 *
 * @param {number} index
 * @param {string} query
 * @returns {HTMLDivElement}
 */
function createRegisterRow(index, query) {
    const label = 'クリップボードを登録';
    const preview = '現在のクリップボードを登録クリップに追加';
    const div = document.createElement('div');
    div.className = 'item action' + (index === selection.index ? ' selected' : '');

    div.innerHTML = `
        <div class="mode-info">
            <span class="label">${highlightText(label, query)}</span>
        </div>
        <span class="value-hint">${highlightText(preview, query)}</span>
    `;

    div.onmouseenter = () => selection.setSelectedIndex(index, false);
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
    results.innerHTML = '';
    const query = normalizeQuery(input.value);
    selection.clampSelectedIndex();

    if (filtered.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'empty';
        empty.textContent = items.length === 0
            ? '登録クリップがありません'
            : '該当する登録クリップがありません';
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

    filtered = items.filter((item) => itemMatches(item, query));

    if (isRegisterIndex(previousIndex)) {
        selection.index = filtered.length;
    } else if (previousIndex < filtered.length) {
        selection.index = previousIndex;
    } else {
        selection.index = 0;
    }

    renderList(true);
}

/**
 * 登録クリップを選択して IPC 送信する
 *
 * @param {string} id
 */
function select(id) {
    postIpc('select:' + id);
}

/** クリップボードから新規登録を IPC 送信する */
function register() {
    postIpc('register');
}

/**
 * 登録クリップを削除する
 *
 * @param {string} id
 */
function deleteItem(id) {
    postIpc('delete:' + id);
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
    close();
}

/** Enter キー相当の選択確定処理 */
function activateSelection() {
    if (isRegisterIndex(selection.index)) {
        register();
    } else if (filtered[selection.index]) {
        select(filtered[selection.index].id);
    }
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
    if (handleHomeEndKeys(e, totalRows, selection.setSelectedIndex)) {
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
    if (e.key === 'Delete' && !isRegisterIndex(selection.index) && filtered[selection.index]) {
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

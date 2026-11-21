// ======================================================================
// DOM 取得補助
// ======================================================================

// `getElementById` は `HTMLElement` を返すため、`value` / `checked` / `disabled` を
// 参照する箇所は要素の種類を明示して取得する (型チェック用。実行時は同じ要素を返す)

/**
 * シェル HTML に必ずある要素を ID で取得する
 *
 * 無い場合は埋め込み HTML とスクリプトの不整合のため、例外にする
 *
 * @param {string} id
 * @returns {HTMLElement}
 */
export function requiredById(id) {
    const element = document.getElementById(id);
    if (!element) {
        throw new Error('#' + id + ' がシェル HTML にありません');
    }
    return element;
}

/**
 * 親要素の中からシェル HTML に必ずある子要素を取得する
 *
 * @param {ParentNode} parent
 * @param {string} selector
 * @returns {HTMLElement}
 */
export function requiredChild(parent, selector) {
    /** @type {HTMLElement|null} */
    const element = parent.querySelector(selector);
    if (!element) {
        throw new Error(selector + ' がシェル HTML にありません');
    }
    return element;
}

/**
 * ID で `<input>` / `<textarea>` を取得する
 *
 * @param {string} id
 * @returns {HTMLInputElement|null}
 */
export function inputById(id) {
    return /** @type {HTMLInputElement|null} */ (document.getElementById(id));
}

/**
 * ID で `<select>` を取得する
 *
 * @param {string} id
 * @returns {HTMLSelectElement|null}
 */
export function selectById(id) {
    return /** @type {HTMLSelectElement|null} */ (document.getElementById(id));
}

/**
 * ID で `<button>` を取得する
 *
 * @param {string} id
 * @returns {HTMLButtonElement|null}
 */
export function buttonById(id) {
    return /** @type {HTMLButtonElement|null} */ (document.getElementById(id));
}

/**
 * イベントの発生元を `<input>` として取得する
 *
 * @param {Event} event
 * @returns {HTMLInputElement}
 */
export function eventInput(event) {
    return /** @type {HTMLInputElement} */ (event.target);
}

/**
 * セレクターに一致する `<input>` をすべて取得する
 *
 * @param {string} selector
 * @returns {NodeListOf<HTMLInputElement>}
 */
export function queryInputs(selector) {
    return document.querySelectorAll(selector);
}

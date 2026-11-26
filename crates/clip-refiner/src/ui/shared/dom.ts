// ======================================================================
// DOM 取得補助
// ======================================================================

// `getElementById` は `HTMLElement` を返すため、`value` / `checked` / `disabled` を
// 参照する箇所は要素の種類を明示して取得する (型チェック用。実行時は同じ要素を返す)

/**
 * シェル HTML に必ずある要素を ID で取得する
 *
 * 無い場合は埋め込み HTML とスクリプトの不整合のため、例外にする
 */
export function requiredById(id: string): HTMLElement {
    const element = document.getElementById(id);
    if (!element) {
        throw new Error('#' + id + ' がシェル HTML にありません');
    }
    return element;
}

/**
 * 親要素の中からシェル HTML に必ずある子要素を取得する
 */
export function requiredChild(parent: ParentNode, selector: string): HTMLElement {
    const element: HTMLElement | null = parent.querySelector(selector);
    if (!element) {
        throw new Error(selector + ' がシェル HTML にありません');
    }
    return element;
}

/**
 * ID で `<input>` / `<textarea>` を取得する
 */
export function inputById(id: string): HTMLInputElement | null {
    return document.getElementById(id) as HTMLInputElement | null;
}

/**
 * ID で `<select>` を取得する
 */
export function selectById(id: string): HTMLSelectElement | null {
    return document.getElementById(id) as HTMLSelectElement | null;
}

/**
 * ID で `<button>` を取得する
 */
export function buttonById(id: string): HTMLButtonElement | null {
    return document.getElementById(id) as HTMLButtonElement | null;
}

/**
 * イベントの発生元を `<input>` として取得する
 */
export function eventInput(event: Event): HTMLInputElement {
    return event.target as HTMLInputElement;
}

/**
 * セレクターに一致する `<input>` をすべて取得する
 */
export function queryInputs(selector: string): NodeListOf<HTMLInputElement> {
    return document.querySelectorAll(selector);
}

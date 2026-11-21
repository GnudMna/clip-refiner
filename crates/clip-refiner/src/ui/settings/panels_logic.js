/**
 * 設定画面のパネル / イベント向け DOM 非依存ロジック
 *
 * @namespace SettingsPanelsLogic
 */
var SettingsPanelsLogic = (function () {
    'use strict';

    /**
     * パイプライン行の上下移動・削除を適用した次の配列を返す
     *
     * @param {string[]} pipeline
     * @param {number} index
     * @param {'up'|'down'|'remove'} action
     * @returns {string[]|null} 変更がない場合は `null`
     */
    function applyPipelineAction(pipeline, index, action) {
        if (
            !Array.isArray(pipeline) ||
            Number.isNaN(index) ||
            index < 0 ||
            index >= pipeline.length
        ) {
            return null;
        }

        if (action === 'up') {
            if (index <= 0) {
                return null;
            }
            const next = pipeline.slice();
            const tmp = next[index - 1];
            next[index - 1] = next[index];
            next[index] = tmp;
            return next;
        }

        if (action === 'down') {
            if (index >= pipeline.length - 1) {
                return null;
            }
            const next = pipeline.slice();
            const tmp = next[index + 1];
            next[index + 1] = next[index];
            next[index] = tmp;
            return next;
        }

        if (action === 'remove') {
            return pipeline.filter((_, i) => i !== index);
        }

        return null;
    }

    /**
     * パイプラインへモードを追加した次の配列を返す
     *
     * @param {string[]} pipeline
     * @param {string} modeId
     * @param {number} maxLength
     * @returns {string[]|null} 追加できない場合は `null`
     */
    function appendPipelineMode(pipeline, modeId, maxLength) {
        if (!modeId) {
            return null;
        }
        const current = Array.isArray(pipeline) ? pipeline : [];
        if (current.length >= maxLength) {
            return null;
        }
        return current.concat([modeId]);
    }

    /**
     * サイドバーナビ HTML を組み立てる
     *
     * @param {ReadonlyArray<{ id: string, label: string }>} tabs
     * @param {string} activeTab
     * @param {(text: string) => string} escapeHtml
     * @returns {string}
     */
    function buildSidebarNavInnerHtml(tabs, activeTab, escapeHtml) {
        return tabs
            .map((tab) => {
                const active = tab.id === activeTab ? ' active' : '';
                return (
                    '<button type="button" class="nav-btn' +
                    active +
                    '" data-tab="' +
                    tab.id +
                    '"' +
                    (tab.id === activeTab ? ' aria-current="page"' : '') +
                    '>' +
                    escapeHtml(tab.label) +
                    '</button>'
                );
            })
            .join('');
    }

    /**
     * 登録クリップ 1 行分のテーブル行 HTML を組み立てる
     *
     * @param {{ id: string, label?: string, preview?: string, kind: string, thumbnail?: string }} clip
     * @param {(text: string) => string} escapeHtml
     * @returns {string}
     */
    function renderClipRow(clip, escapeHtml) {
        const label = clip.label || clip.preview || '(無題)';
        const kindBadge =
            clip.kind === 'image'
                ? '<span class="clip-kind clip-kind--image">画像</span>'
                : '<span class="clip-kind clip-kind--text">テキスト</span>';
        let thumbHtml = '';
        if (clip.kind === 'image') {
            if (clip.thumbnail) {
                thumbHtml =
                    '<img class="clip-thumbnail" src="' + escapeHtml(clip.thumbnail) + '" alt="">';
            } else {
                thumbHtml =
                    '<span class="clip-thumbnail clip-thumbnail--placeholder" aria-hidden="true">🖼</span>';
            }
        }
        const previewHtml =
            clip.preview && clip.preview !== label
                ? '<span class="clip-preview">' + escapeHtml(clip.preview) + '</span>'
                : '';
        return (
            '<tr><td><div class="clip-cell">' +
            thumbHtml +
            '<div class="clip-meta">' +
            '<div class="clip-label-row"><span class="clip-label">' +
            escapeHtml(label) +
            '</span>' +
            kindBadge +
            '</div>' +
            previewHtml +
            '</div></div></td><td class="list-actions">' +
            '<button type="button" class="ghost-btn clip-copy" data-index="' +
            escapeHtml(clip.id) +
            '">コピー</button>' +
            '<button type="button" class="icon-btn clip-delete" data-index="' +
            escapeHtml(clip.id) +
            '" title="削除">×</button>' +
            '</td></tr>'
        );
    }

    /**
     * お気に入り一覧のテーブル行 HTML を組み立てる
     *
     * @param {string[]} favorites
     * @param {(modeId: string) => string} modeLabel
     * @param {(text: string) => string} escapeHtml
     * @returns {string}
     */
    function buildFavoriteRowsHtml(favorites, modeLabel, escapeHtml) {
        return favorites
            .map((modeId) => {
                return (
                    '<tr><td>' +
                    escapeHtml(modeLabel(modeId)) +
                    '</td><td class="list-actions">' +
                    '<button type="button" class="icon-btn favorite-up" data-mode="' +
                    escapeHtml(modeId) +
                    '" title="上へ">↑</button>' +
                    '<button type="button" class="icon-btn favorite-down" data-mode="' +
                    escapeHtml(modeId) +
                    '" title="下へ">↓</button>' +
                    '<button type="button" class="icon-btn favorite-remove" data-mode="' +
                    escapeHtml(modeId) +
                    '" title="削除">×</button>' +
                    '</td></tr>'
                );
            })
            .join('');
    }

    /**
     * お気に入り未登録の追加候補モードを返す
     *
     * @param {ReadonlyArray<{ id: string, label: string }>} modes
     * @param {string[]} favorites
     * @returns {Array<{ id: string, label: string }>}
     */
    function availableFavoriteModes(modes, favorites) {
        const favoriteSet = new Set(favorites || []);
        return (modes || []).filter((mode) => !favoriteSet.has(mode.id));
    }

    /**
     * お気に入りスロット入力の表示値を求める
     *
     * @param {string[]|undefined} slots
     * @param {number} index
     * @returns {{ value: string, placeholder: string }}
     */
    function favoriteSlotDisplay(slots, index) {
        const placeholder = index < 9 ? 'Alt+Shift+' + (index + 1) : 'Alt+Shift+F' + (index - 8);
        const list = Array.isArray(slots) ? slots : [];
        const custom = index < list.length ? list[index] : undefined;
        return {
            value: custom === undefined ? placeholder : custom,
            placeholder,
        };
    }

    /**
     * コンテンツクリックから委譲アクションを解決する
     *
     * @param {{ classList: { contains: (name: string) => boolean }, dataset: Record<string, string> }} button
     * @returns {{ type: string, mode?: string, direction?: string, index?: number }|null}
     */
    function resolveContentClickAction(button) {
        if (
            button.classList.contains('pipeline-up') ||
            button.classList.contains('pipeline-down') ||
            button.classList.contains('pipeline-remove')
        ) {
            const index = Number(button.dataset.index);
            if (Number.isNaN(index)) {
                return null;
            }
            let action = 'remove';
            if (button.classList.contains('pipeline-up')) {
                action = 'up';
            } else if (button.classList.contains('pipeline-down')) {
                action = 'down';
            }
            return { type: 'pipeline', action, index };
        }

        if (button.classList.contains('favorite-remove')) {
            return { type: 'favorite-toggle', mode: button.dataset.mode };
        }
        if (button.classList.contains('favorite-up')) {
            return { type: 'favorite-move', mode: button.dataset.mode, direction: 'up' };
        }
        if (button.classList.contains('favorite-down')) {
            return { type: 'favorite-move', mode: button.dataset.mode, direction: 'down' };
        }
        if (button.classList.contains('clip-copy')) {
            return { type: 'clip-copy', index: Number(button.dataset.index) };
        }
        if (button.classList.contains('clip-delete')) {
            return { type: 'clip-delete', index: Number(button.dataset.index) };
        }
        return null;
    }

    /**
     * 登録クリップボタンの表示状態を返す
     *
     * @param {boolean} pending
     * @returns {{ className: string, label: string, disabled: boolean }}
     */
    function clipRegisterButtonState(pending) {
        return {
            className: pending ? 'ghost-btn is-loading' : 'ghost-btn',
            label: pending ? '登録中...' : 'クリップボードから登録',
            disabled: !!pending,
        };
    }

    return {
        applyPipelineAction,
        appendPipelineMode,
        buildSidebarNavInnerHtml,
        renderClipRow,
        buildFavoriteRowsHtml,
        availableFavoriteModes,
        favoriteSlotDisplay,
        resolveContentClickAction,
        clipRegisterButtonState,
    };
})();

if (typeof module !== 'undefined' && module.exports) {
    module.exports = SettingsPanelsLogic;
}

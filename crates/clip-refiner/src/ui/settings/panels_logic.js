/**
 * 設定画面のパネル / イベント向け DOM 非依存ロジック
 *
 * @namespace SettingsPanelsLogic
 */
const SettingsPanelsLogic = (function () {
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
     * タブ選択状態に対応する `aria-selected` / `tabindex` を返す
     *
     * @param {boolean} isActive
     * @returns {{ ariaSelected: 'true'|'false', tabIndex: '0'|'-1' }}
     */
    function tabSelectionState(isActive) {
        return {
            ariaSelected: isActive ? 'true' : 'false',
            tabIndex: isActive ? '0' : '-1',
        };
    }

    /**
     * サイドバー tablist 内のタブボタン HTML を組み立てる
     *
     * @param {ReadonlyArray<{ id: string, label: string }>} tabs
     * @param {string} activeTab
     * @param {(text: string) => string} escapeHtml
     * @returns {string}
     */
    function buildSidebarNavInnerHtml(tabs, activeTab, escapeHtml) {
        return tabs
            .map((tab) => {
                const isActive = tab.id === activeTab;
                const active = isActive ? ' active' : '';
                const selection = tabSelectionState(isActive);
                return (
                    '<button type="button" role="tab" class="nav-btn' +
                    active +
                    '" id="tab-' +
                    tab.id +
                    '" data-tab="' +
                    tab.id +
                    '" aria-selected="' +
                    selection.ariaSelected +
                    '" aria-controls="panel-' +
                    tab.id +
                    '" tabindex="' +
                    selection.tabIndex +
                    '">' +
                    escapeHtml(tab.label) +
                    '</button>'
                );
            })
            .join('');
    }

    /**
     * 縦方向 tablist の矢印 / Home / End から次のタブインデックスを計算する
     *
     * 左右矢印も上下と同じ移動として扱う (縦リストでも片手操作しやすくするため)
     *
     * @param {number} currentIndex
     * @param {number} tabCount
     * @param {string} key
     * @returns {number|null} 移動先インデックス。対象外キーは `null`
     */
    function nextTabIndexForKey(currentIndex, tabCount, key) {
        if (tabCount <= 0 || currentIndex < 0 || currentIndex >= tabCount) {
            return null;
        }
        if (key === 'ArrowDown' || key === 'ArrowRight') {
            return (currentIndex + 1) % tabCount;
        }
        if (key === 'ArrowUp' || key === 'ArrowLeft') {
            return (currentIndex - 1 + tabCount) % tabCount;
        }
        if (key === 'Home') {
            return 0;
        }
        if (key === 'End') {
            return tabCount - 1;
        }
        return null;
    }

    /**
     * 登録クリップ 1 行分のテーブル行 HTML を組み立てる
     *
     * @param {{ id: string, label?: string, preview?: string, kind: string, template?: boolean, counter?: number, thumbnail?: string }} clip
     * @param {(text: string) => string} escapeHtml
     * @param {number} index - 一覧内の位置 (先頭は上へ移動を無効にする)
     * @param {number} count - 一覧の件数 (末尾は下へ移動を無効にする)
     * @returns {string}
     */
    function renderClipRow(clip, escapeHtml, index, count) {
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
        // テンプレート展開はテキストのみ。押下で現在と逆の状態を要求し、有効状態はボタンの強調で示す
        // 画像行に無いボタンを先頭に置き、↑↓・コピー・削除の位置を行間で揃える
        const templateButton =
            clip.kind === 'image'
                ? ''
                : '<button type="button" class="ghost-btn clip-template' +
                  (clip.template ? ' is-active' : '') +
                  '" data-index="' +
                  escapeHtml(clip.id) +
                  '" data-enabled="' +
                  (clip.template ? 'false' : 'true') +
                  '" aria-pressed="' +
                  (clip.template ? 'true' : 'false') +
                  '" title="コピー時に {date} などを展開する">テンプレート</button>';
        // 本文とラベルの編集もテキストのみ。テンプレートと同じ理由で ↑↓ の前に置く
        const editButton =
            clip.kind === 'image'
                ? ''
                : '<button type="button" class="ghost-btn clip-edit" data-index="' +
                  escapeHtml(clip.id) +
                  '" title="ラベルと本文を編集する">編集</button>';
        // 連番は払い出し済みのテキストだけに出す。リセットはその横のボタンで行う
        const hasCounter = clip.kind !== 'image' && clip.counter > 0;
        const counterBadge = hasCounter
            ? '<span class="clip-kind clip-counter-badge" title="{counter} で最後に使った連番">連番 ' +
              escapeHtml(String(clip.counter)) +
              '</span>'
            : '';
        const counterResetButton = hasCounter
            ? '<button type="button" class="ghost-btn clip-counter-reset" data-index="' +
              escapeHtml(clip.id) +
              '" title="次のコピーで {counter} を 1 から数え直す">連番リセット</button>'
            : '';
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
            counterBadge +
            '</div>' +
            previewHtml +
            '</div></div></td><td class="list-actions">' +
            counterResetButton +
            templateButton +
            editButton +
            '<button type="button" class="icon-btn clip-up" data-index="' +
            escapeHtml(clip.id) +
            '" title="上へ"' +
            (index === 0 ? ' disabled' : '') +
            '>↑</button>' +
            '<button type="button" class="icon-btn clip-down" data-index="' +
            escapeHtml(clip.id) +
            '" title="下へ"' +
            (index === count - 1 ? ' disabled' : '') +
            '>↓</button>' +
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
     * @returns {{ type: string, mode?: string, direction?: string, index?: number, enabled?: boolean }|null}
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

        if (
            button.classList.contains('regex-preset-up') ||
            button.classList.contains('regex-preset-down') ||
            button.classList.contains('regex-preset-remove') ||
            button.classList.contains('regex-preset-select') ||
            button.classList.contains('regex-step-up') ||
            button.classList.contains('regex-step-down') ||
            button.classList.contains('regex-step-remove')
        ) {
            return { type: 'regex-action', button };
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
        if (button.classList.contains('clip-up')) {
            return { type: 'clip-move', index: Number(button.dataset.index), direction: 'up' };
        }
        if (button.classList.contains('clip-down')) {
            return { type: 'clip-move', index: Number(button.dataset.index), direction: 'down' };
        }
        if (button.classList.contains('clip-template')) {
            return {
                type: 'clip-template',
                index: Number(button.dataset.index),
                enabled: button.dataset.enabled === 'true',
            };
        }
        if (button.classList.contains('clip-counter-reset')) {
            return { type: 'clip-counter-reset', index: Number(button.dataset.index) };
        }
        if (button.classList.contains('clip-edit')) {
            return { type: 'clip-edit', index: Number(button.dataset.index) };
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

    /**
     * 登録クリップの編集内容を保存前に検証する
     *
     * 本文のバイト数上限は Rust 側で検証する。ラベルは空でもよい (本文から補完される)
     *
     * @param {string} label
     * @param {string} text
     * @param {number} maxLabelChars
     * @returns {{ ok: true } | { ok: false, field: 'label'|'text', message: string }}
     */
    function validateClipEdit(label, text, maxLabelChars) {
        if (String(text).trim() === '') {
            return { ok: false, field: 'text', message: '本文を入力してください' };
        }
        if (Array.from(String(label).trim()).length > maxLabelChars) {
            return {
                ok: false,
                field: 'label',
                message: 'ラベルは ' + maxLabelChars + ' 文字以内にしてください',
            };
        }
        return { ok: true };
    }

    /**
     * 編集中の登録クリップが一覧上で開いた時点のままか判定する
     *
     * 他の操作で削除・並べ替えされると、保存先のインデックスがずれて別のクリップを上書きするため
     *
     * @param {{ index: number, label: string, preview: string }|null} editor
     * @param {Array<{ label?: string, preview?: string, kind: string }>} clips
     * @returns {boolean}
     */
    function clipEditorTargetUnchanged(editor, clips) {
        if (!editor || !Array.isArray(clips)) {
            return false;
        }
        const clip = clips[editor.index];
        return (
            !!clip &&
            clip.kind === 'text' &&
            (clip.label ?? '') === editor.label &&
            (clip.preview ?? '') === editor.preview
        );
    }

    /**
     * バイト数を読みやすい文字列にする
     *
     * @param {number|undefined|null} bytes
     * @returns {string}
     */
    function formatBytes(bytes) {
        const value = Number(bytes);
        if (!Number.isFinite(value) || value < 0) {
            return '';
        }
        if (value < 1024) {
            return value + ' B';
        }
        if (value < 1024 * 1024) {
            return (value / 1024).toFixed(1) + ' KB';
        }
        return (value / (1024 * 1024)).toFixed(1) + ' MB';
    }

    /**
     * 更新ステータス表示 HTML を組み立てる
     *
     * @param {Record<string, unknown>|null|undefined} status
     * @param {boolean} updateBusy
     * @param {(text: string) => string} escapeHtml
     * @returns {string}
     */
    function renderUpdateStatusHtml(status, updateBusy, escapeHtml) {
        if (!status || typeof status !== 'object') {
            return '<p id="update-status" class="hint">「更新を確認」で最新版を調べます</p>';
        }

        const kind = status.status;
        if (kind === 'up-to-date') {
            return (
                '<p id="update-status" class="hint">最新版です (v' +
                escapeHtml(String(status.latestVersion ?? '')) +
                ')</p>'
            );
        }
        if (kind === 'available') {
            const sizeLabel = formatBytes(status.size);
            const sizeHtml = sizeLabel ? ' / ' + escapeHtml(sizeLabel) : '';
            return (
                '' +
                '<p id="update-status" class="hint">v' +
                escapeHtml(String(status.latestVersion ?? '')) +
                ' が利用可能です' +
                sizeHtml +
                '</p>' +
                '<div class="update-actions">' +
                '<button type="button" id="apply-update-btn" class="primary-btn"' +
                (updateBusy ? ' disabled' : '') +
                '>' +
                (updateBusy ? '適用中...' : 'ダウンロードして適用') +
                '</button>' +
                '</div>'
            );
        }
        if (kind === 'msi-started' || kind === 'portable-extracted') {
            return (
                '<p id="update-status" class="hint">' +
                escapeHtml(String(status.message ?? '')) +
                '</p>'
            );
        }
        if (kind === 'error' || kind === 'busy') {
            return (
                '<p id="update-status" class="hint">' +
                escapeHtml(String(status.message ?? '更新処理に失敗しました')) +
                '</p>'
            );
        }
        return '<p id="update-status" class="hint"></p>';
    }

    return {
        applyPipelineAction,
        appendPipelineMode,
        tabSelectionState,
        buildSidebarNavInnerHtml,
        nextTabIndexForKey,
        renderClipRow,
        buildFavoriteRowsHtml,
        availableFavoriteModes,
        favoriteSlotDisplay,
        resolveContentClickAction,
        clipRegisterButtonState,
        validateClipEdit,
        clipEditorTargetUnchanged,
        formatBytes,
        renderUpdateStatusHtml,
    };
})();

export default SettingsPanelsLogic;

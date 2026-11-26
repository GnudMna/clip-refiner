import type { ClipSelectorItem } from '../selector/clip_selector_logic.ts';
import type { MoveDirection } from '../ipc.ts';

/**
 * 委譲クリックの対象ボタン (DOM 非依存で扱う最小限の形)
 */
type ActionButton = {
    classList: { contains: (name: string) => boolean };
    dataset: Record<string, string | undefined>;
};

/**
 * コンテンツクリックから解決した委譲アクション
 *
 * `clip-*` は IPC の `IpcClipTarget` と同じく `index` と識別トークンを持つ
 */
type ContentClickAction =
    | { type: 'pipeline'; action: 'up' | 'down' | 'remove'; index: number }
    | { type: 'regex-action'; button: ActionButton }
    | { type: 'favorite-toggle'; mode: string }
    | { type: 'favorite-move'; mode: string; direction: MoveDirection }
    | { type: 'clip-move'; index: number; token: string; direction: MoveDirection }
    | { type: 'clip-template'; index: number; token: string; enabled: boolean }
    | {
          type: 'clip-counter-reset' | 'clip-edit' | 'clip-copy' | 'clip-delete';
          index: number;
          token: string;
      };

/**
 * 設定画面のパネル / イベント向け DOM 非依存ロジック
 */
const SettingsPanelsLogic = (function () {
    'use strict';

    /**
     * パイプライン行の上下移動・削除を適用した次の配列を返す
     *
     * @returns 変更がない場合は `null`
     */
    function applyPipelineAction(
        pipeline: string[],
        index: number,
        action: 'up' | 'down' | 'remove'
    ): string[] | null {
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
            const prev = next[index - 1];
            const cur = next[index];
            if (prev === undefined || cur === undefined) {
                return null;
            }
            next[index - 1] = cur;
            next[index] = prev;
            return next;
        }

        if (action === 'down') {
            if (index >= pipeline.length - 1) {
                return null;
            }
            const next = pipeline.slice();
            const following = next[index + 1];
            const cur = next[index];
            if (following === undefined || cur === undefined) {
                return null;
            }
            next[index + 1] = cur;
            next[index] = following;
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
     * @returns 追加できない場合は `null`
     */
    function appendPipelineMode(
        pipeline: string[],
        modeId: string,
        maxLength: number
    ): string[] | null {
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
     */
    function tabSelectionState(isActive: boolean): {
        ariaSelected: 'true' | 'false';
        tabIndex: '0' | '-1';
    } {
        return {
            ariaSelected: isActive ? 'true' : 'false',
            tabIndex: isActive ? '0' : '-1',
        };
    }

    /**
     * サイドバー tablist 内のタブボタン HTML を組み立てる
     */
    function buildSidebarNavInnerHtml(
        tabs: ReadonlyArray<{ id: string; label: string }>,
        activeTab: string,
        escapeHtml: (text: string) => string
    ): string {
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
     * @returns 移動先インデックス。対象外キーは `null`
     */
    function nextTabIndexForKey(
        currentIndex: number,
        tabCount: number,
        key: string
    ): number | null {
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
     * @param index - 一覧内の位置 (先頭は上へ移動を無効にする)
     * @param count - 一覧の件数 (末尾は下へ移動を無効にする)
     */
    function renderClipRow(
        clip: ClipSelectorItem,
        escapeHtml: (text: string) => string,
        index: number,
        count: number
    ): string {
        const label = clip.label || clip.preview || '(無題)';
        // 操作は表示時のインデックスに加えて識別トークンを送り、並びが変わった後の誤操作を防ぐ
        const indexAttrEnd = '" data-token="' + escapeHtml(clip.token ?? '') + '"';
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
                  indexAttrEnd +
                  ' data-enabled="' +
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
                  indexAttrEnd +
                  ' title="ラベルと本文を編集する">編集</button>';
        // 連番は払い出し済みのテキストだけに出す。リセットはその横のボタンで行う
        const hasCounter = clip.kind !== 'image' && (clip.counter ?? 0) > 0;
        const counterBadge = hasCounter
            ? '<span class="clip-kind clip-counter-badge" title="{counter} で最後に使った連番">連番 ' +
              escapeHtml(String(clip.counter)) +
              '</span>'
            : '';
        const counterResetButton = hasCounter
            ? '<button type="button" class="ghost-btn clip-counter-reset" data-index="' +
              escapeHtml(clip.id) +
              indexAttrEnd +
              ' title="次のコピーで {counter} を 1 から数え直す">連番リセット</button>'
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
            indexAttrEnd +
            ' title="上へ"' +
            (index === 0 ? ' disabled' : '') +
            '>↑</button>' +
            '<button type="button" class="icon-btn clip-down" data-index="' +
            escapeHtml(clip.id) +
            indexAttrEnd +
            ' title="下へ"' +
            (index === count - 1 ? ' disabled' : '') +
            '>↓</button>' +
            '<button type="button" class="ghost-btn clip-copy" data-index="' +
            escapeHtml(clip.id) +
            indexAttrEnd +
            '>コピー</button>' +
            '<button type="button" class="icon-btn clip-delete" data-index="' +
            escapeHtml(clip.id) +
            indexAttrEnd +
            ' title="削除">×</button>' +
            '</td></tr>'
        );
    }

    /**
     * お気に入り一覧のテーブル行 HTML を組み立てる
     */
    function buildFavoriteRowsHtml(
        favorites: string[],
        modeLabel: (modeId: string) => string,
        escapeHtml: (text: string) => string
    ): string {
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
     */
    function availableFavoriteModes(
        modes: ReadonlyArray<{ id: string; label: string }>,
        favorites: string[]
    ): Array<{ id: string; label: string }> {
        const favoriteSet = new Set(favorites || []);
        return (modes || []).filter((mode) => !favoriteSet.has(mode.id));
    }

    /**
     * お気に入りスロット入力の表示値を求める
     */
    function favoriteSlotDisplay(
        slots: string[] | undefined,
        index: number
    ): { value: string; placeholder: string } {
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
     */
    function resolveContentClickAction(button: ActionButton): ContentClickAction | null {
        if (
            button.classList.contains('pipeline-up') ||
            button.classList.contains('pipeline-down') ||
            button.classList.contains('pipeline-remove')
        ) {
            const index = Number(button.dataset.index);
            if (Number.isNaN(index)) {
                return null;
            }
            let action: 'up' | 'down' | 'remove' = 'remove';
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

        const mode = button.dataset.mode;
        if (mode !== undefined) {
            if (button.classList.contains('favorite-remove')) {
                return { type: 'favorite-toggle', mode };
            }
            if (button.classList.contains('favorite-up')) {
                return { type: 'favorite-move', mode, direction: 'up' };
            }
            if (button.classList.contains('favorite-down')) {
                return { type: 'favorite-move', mode, direction: 'down' };
            }
        }

        // 登録クリップの操作は識別トークンが無いと Rust が受け付けないため、送らずに打ち切る
        const token = button.dataset.token;
        if (token === undefined) {
            return null;
        }
        const index = Number(button.dataset.index);
        if (button.classList.contains('clip-up')) {
            return { type: 'clip-move', index, token, direction: 'up' };
        }
        if (button.classList.contains('clip-down')) {
            return { type: 'clip-move', index, token, direction: 'down' };
        }
        if (button.classList.contains('clip-template')) {
            return {
                type: 'clip-template',
                index,
                token,
                enabled: button.dataset.enabled === 'true',
            };
        }
        if (button.classList.contains('clip-counter-reset')) {
            return { type: 'clip-counter-reset', index, token };
        }
        if (button.classList.contains('clip-edit')) {
            return { type: 'clip-edit', index, token };
        }
        if (button.classList.contains('clip-copy')) {
            return { type: 'clip-copy', index, token };
        }
        if (button.classList.contains('clip-delete')) {
            return { type: 'clip-delete', index, token };
        }
        return null;
    }

    /**
     * 登録クリップボタンの表示状態を返す
     */
    function clipRegisterButtonState(pending: boolean): {
        className: string;
        label: string;
        disabled: boolean;
    } {
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
     */
    function validateClipEdit(
        label: string,
        text: string,
        maxLabelChars: number
    ): { ok: true } | { ok: false; field: 'label' | 'text'; message: string } {
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
     */
    function clipEditorTargetUnchanged(
        editor: { index: number; token?: string; label: string; preview: string } | null,
        clips: Array<{ label?: string; preview?: string; kind: string; token?: string }> | undefined
    ): boolean {
        if (!editor || !Array.isArray(clips)) {
            return false;
        }
        const clip = clips[editor.index];
        return (
            !!clip &&
            clip.kind === 'text' &&
            (editor.token === undefined || clip.token === editor.token) &&
            (clip.label ?? '') === editor.label &&
            (clip.preview ?? '') === editor.preview
        );
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
    };
})();

export default SettingsPanelsLogic;

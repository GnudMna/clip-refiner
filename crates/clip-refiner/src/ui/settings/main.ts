// ======================================================================
// 設定画面 描画・Rust 連携エントリ
// ======================================================================

import { requiredById } from '../shared/dom.ts';
import {
    bindClipEditor,
    closeClipEditorIfStale,
    isClipEditorOpen,
    onClipEditResult,
    showClipEditor,
} from './dialogs/clip_editor.ts';
import { bindConfirmDialog } from './dialogs/confirm_dialog.ts';
import { ctx } from './ctx.ts';
import { bindActivePanelEvents, bindContentDelegation } from './events.ts';
import { closeWindow, flushPendingApply, openConfigFile, showApplyFeedback } from './ipc_apply.ts';
import { PANEL_RENDERERS, ensureSidebar } from './panels.ts';
import {
    handleRegexPreview,
    readRegexForApply,
    readRegexSampleFromDom,
} from './regex/regex_feedback.ts';
import {
    invalidateAllPanels,
    invalidatePanel,
    isContentEditing,
    showActivePanel,
    syncHotkeyInputsFromState,
    syncServerFields,
    withPreservedScroll,
} from './shared.ts';

import SettingsSharedLogic from './shared_logic.ts';

import type { ClipSelectorItem } from '../selector/clip_selector_logic.ts';
import type {
    FavoritesAndShortcutsPayload,
    LoadSettingsOptions,
    SettingsSnapshot,
} from '../ipc.ts';

/**
 * 指定タブのパネル HTML を生成する (escapeHtml 等でサニタイズ済み)
 */
function resolvePanelInnerHtml(tabId: string): string {
    const renderer = PANEL_RENDERERS[tabId] ?? PANEL_RENDERERS.general;
    return renderer ? renderer() : '';
}

/**
 * アクティブタブのパネルを描画する
 *
 * @param force - キャッシュを無視して再描画するか
 */
function renderActivePanel(force: boolean = false): void {
    bindContentDelegation();
    const content = requiredById('content');
    const panelId = 'panel-' + ctx.activeTab;

    if (!force && ctx.renderedPanels.has(ctx.activeTab)) {
        showActivePanel();
        return;
    }

    const existing = document.getElementById(panelId);
    if (existing) {
        existing.outerHTML = resolvePanelInnerHtml(ctx.activeTab);
    } else {
        content.insertAdjacentHTML('beforeend', resolvePanelInnerHtml(ctx.activeTab));
    }

    if (!ctx.boundPanels.has(ctx.activeTab)) {
        bindActivePanelEvents();
        ctx.boundPanels.add(ctx.activeTab);
    }

    ctx.renderedPanels.add(ctx.activeTab);
    showActivePanel();
}

/** アクティブタブのパネルを破棄して再描画する (スクロール位置は保つ) */
export function refreshActivePanel(): void {
    withPreservedScroll(() => {
        invalidatePanel(ctx.activeTab);
        renderActivePanel(true);
    });
}

/**
 * サイドバーとアクティブパネルを描画する
 */
export function render(forcePanel: boolean = false): void {
    ensureSidebar();
    renderActivePanel(forcePanel);
}

ctx.render = render;
ctx.refreshActivePanel = refreshActivePanel;

/**
 * Rust から設定スナップショットを受信して UI を更新する
 */
window.loadSettings = function loadSettings(
    snapshot: SettingsSnapshot,
    options?: LoadSettingsOptions
) {
    const opts = options || {};
    const wasEditing = isContentEditing();
    const editingTab = wasEditing ? ctx.activeTab : null;

    if (editingTab === 'regex' && !opts.externalSync) {
        snapshot = Object.assign({}, snapshot, {
            regex: readRegexForApply(),
            regexPreviewSample: readRegexSampleFromDom(),
        });
    }

    ctx.state = snapshot;
    closeClipEditorIfStale(snapshot.clips);

    if (opts.externalSync) {
        showApplyFeedback({
            ok: true,
            message: '設定ファイルが外部で更新されました',
        });
    }

    if (ctx.clipRegisterPending && ctx.activeTab !== 'clipboard') {
        ctx.clipRegisterPending = false;
        ensureSidebar();
        syncServerFields();
        return;
    }

    if (
        SettingsSharedLogic.shouldFullRenderOnLoad(
            opts,
            wasEditing,
            ctx.activeTab,
            ctx.clipRegisterPending
        )
    ) {
        ctx.clipRegisterPending = false;
        withPreservedScroll(() => {
            invalidateAllPanels();
            render(true);
        });
        return;
    }

    ctx.clipRegisterPending = false;
    ensureSidebar();
    syncServerFields();
};

/**
 * 登録クリップ一覧の差分更新
 */
window.updateClips = function updateClips(clips: ClipSelectorItem[]) {
    if (!ctx.state) {
        return;
    }
    ctx.state.clips = clips;
    ctx.clipRegisterPending = false;
    closeClipEditorIfStale(clips);
    if (ctx.activeTab === 'clipboard') {
        refreshActivePanel();
    }
};

window.showClipEditor = showClipEditor;
window.onClipEditResult = onClipEditResult;

/**
 * お気に入りとショートカット一覧の差分更新
 */
window.updateFavoritesAndShortcuts = function updateFavoritesAndShortcuts(
    payload: FavoritesAndShortcutsPayload
) {
    if (!ctx.state) {
        return;
    }
    const previousFavoriteCount = (ctx.state.favoriteModes || []).length;
    const nextFavoriteModes = payload.favoriteModes || [];
    const wasEditingShortcuts =
        ctx.activeTab === 'shortcuts' &&
        Boolean(document.activeElement?.classList?.contains('shortcut-input'));

    ctx.state.favoriteModes = nextFavoriteModes;
    if (payload.hotkeys) {
        ctx.state.hotkeys = payload.hotkeys;
    }
    ctx.state.shortcuts = payload.shortcuts || [];

    if (ctx.activeTab === 'convert') {
        refreshActivePanel();
    } else if (ctx.activeTab === 'shortcuts') {
        const action = SettingsSharedLogic.resolveShortcutsDiffUpdate({
            wasEditingShortcuts,
            previousFavoriteCount,
            nextFavoriteCount: nextFavoriteModes.length,
        });
        if (action === 'syncInputs') {
            syncHotkeyInputsFromState();
        } else {
            refreshActivePanel();
        }
    }
};

/**
 * 保留中の設定パッチを即時送信する
 *
 * Rust から `window.flushPendingApply()` として呼び出す
 */
window.flushPendingApply = flushPendingApply;

/**
 * 保留パッチをフラッシュして設定ウィンドウを閉じる
 *
 * Rust から `window.closeWindow()` として呼び出す (タイトルバーの閉じるボタン)
 */
window.closeWindow = closeWindow;

/**
 * 設定適用のフィードバックを表示する
 *
 * Rust から `window.onApplyFeedback(payload)` として呼び出す
 */
window.onApplyFeedback = showApplyFeedback;

/**
 * 正規表現プレビュー IPC 応答を処理する
 *
 * Rust から `window.onRegexPreview(payload)` として呼び出す
 */
window.onRegexPreview = handleRegexPreview;

bindConfirmDialog();
// Escape をこのダイアログで消費させるため、下の設定ウィンドウ用 keydown より先に登録する
bindClipEditor();
requiredById('open-config-btn').addEventListener('click', openConfigFile);
requiredById('close-btn').addEventListener('click', closeWindow);

document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') {
        return;
    }
    const dialog = document.getElementById('confirm-dialog');
    if (
        !SettingsSharedLogic.shouldCloseSettingsOnEscape({
            dialogVisible: Boolean(dialog && !dialog.hidden) || isClipEditorOpen(),
            isEditing: isContentEditing(),
        })
    ) {
        return;
    }
    closeWindow();
});

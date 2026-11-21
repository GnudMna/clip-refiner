// ======================================================================
// 設定画面 描画・Rust 連携エントリ
// ======================================================================

/**
 * 指定タブのパネル HTML を生成する (escapeHtml 等でサニタイズ済み)
 *
 * @param {string} tabId
 * @returns {string}
 */
function resolvePanelInnerHtml(tabId) {
    const renderer = PANEL_RENDERERS[tabId] || PANEL_RENDERERS.general;
    return renderer();
}

/**
 * アクティブタブのパネルを描画する
 *
 * @param {boolean} [force=false] - キャッシュを無視して再描画するか
 */
function renderActivePanel(force = false) {
    bindContentDelegation();
    const content = document.getElementById('content');
    const panelId = 'panel-' + activeTab;

    if (!force && renderedPanels.has(activeTab)) {
        showActivePanel();
        return;
    }

    const existing = document.getElementById(panelId);
    if (existing) {
        existing.outerHTML = resolvePanelInnerHtml(activeTab);
    } else {
        content.insertAdjacentHTML('beforeend', resolvePanelInnerHtml(activeTab));
    }

    if (!boundPanels.has(activeTab)) {
        bindActivePanelEvents();
        boundPanels.add(activeTab);
    }

    renderedPanels.add(activeTab);
    showActivePanel();
}

/** アクティブタブのパネルを破棄して再描画する */
function refreshActivePanel() {
    invalidatePanel(activeTab);
    renderActivePanel(true);
}

/**
 * サイドバーとアクティブパネルを描画する
 *
 * @param {boolean} [forcePanel=false]
 */
function render(forcePanel = false) {
    ensureSidebar();
    renderActivePanel(forcePanel);
}

/**
 * Rust から設定スナップショットを受信して UI を更新する
 *
 * @param {Record<string, unknown>} snapshot
 * @param {{ force?: boolean, externalSync?: boolean }} [options]
 */
window.loadSettings = function loadSettings(snapshot, options) {
    const opts = options || {};
    const wasEditing = isContentEditing();
    const editingTab = wasEditing ? activeTab : null;

    if (editingTab === 'regex' && !opts.externalSync) {
        snapshot = Object.assign({}, snapshot, {
            regex: readRegexFromDom(),
        });
    }

    state = snapshot;

    if (opts.externalSync) {
        showApplyFeedback({
            ok: true,
            message: '設定ファイルが外部で更新されました',
        });
    }

    if (clipRegisterPending && activeTab !== 'clips') {
        clipRegisterPending = false;
        ensureSidebar();
        syncServerFields();
        return;
    }

    if (
        SettingsSharedLogic.shouldFullRenderOnLoad(opts, wasEditing, activeTab, clipRegisterPending)
    ) {
        clipRegisterPending = false;
        invalidateAllPanels();
        render(true);
        return;
    }

    clipRegisterPending = false;
    ensureSidebar();
    syncServerFields();
};

/**
 * 登録クリップ一覧の差分更新
 *
 * @param {Array<Record<string, unknown>>} clips
 */
window.updateClips = function updateClips(clips) {
    if (!state) {
        return;
    }
    state.clips = clips;
    clipRegisterPending = false;
    if (activeTab === 'clips') {
        refreshActivePanel();
    }
};

/**
 * お気に入りとショートカット一覧の差分更新
 *
 * @param {{ favoriteModes?: string[], hotkeys?: Record<string, unknown>, shortcuts?: Array<{ binding: string, label: string }> }} payload
 */
window.updateFavoritesAndShortcuts = function updateFavoritesAndShortcuts(payload) {
    if (!state) {
        return;
    }
    const previousFavoriteCount = (state.favoriteModes || []).length;
    const nextFavoriteModes = payload.favoriteModes || [];
    const wasEditingShortcuts =
        activeTab === 'shortcuts' && document.activeElement?.classList?.contains('shortcut-input');

    state.favoriteModes = nextFavoriteModes;
    if (payload.hotkeys) {
        state.hotkeys = payload.hotkeys;
    }
    state.shortcuts = payload.shortcuts || [];

    if (activeTab === 'favorites') {
        refreshActivePanel();
    } else if (activeTab === 'shortcuts') {
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
 *
 * @type {typeof flushPendingApply}
 */
window.flushPendingApply = flushPendingApply;

/**
 * 保留パッチをフラッシュして設定ウィンドウを閉じる
 *
 * Rust から `window.closeWindow()` として呼び出す (タイトルバーの閉じるボタン)
 *
 * @type {typeof closeWindow}
 */
window.closeWindow = closeWindow;

/**
 * 設定適用のフィードバックを表示する
 *
 * Rust から `window.onApplyFeedback(payload)` として呼び出す
 *
 * @type {typeof showApplyFeedback}
 */
window.onApplyFeedback = showApplyFeedback;

/**
 * 正規表現プレビュー IPC 応答を処理する
 *
 * Rust から `window.onRegexPreview(payload)` として呼び出す
 *
 * @type {typeof handleRegexPreview}
 */
window.onRegexPreview = handleRegexPreview;

bindConfirmDialog();
document.getElementById('open-config-btn').addEventListener('click', openConfigFile);
document.getElementById('close-btn').addEventListener('click', closeWindow);

document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') {
        return;
    }
    const dialog = document.getElementById('confirm-dialog');
    if (
        !SettingsSharedLogic.shouldCloseSettingsOnEscape({
            dialogVisible: Boolean(dialog && !dialog.hidden),
            isEditing: isContentEditing(),
        })
    ) {
        return;
    }
    closeWindow();
});

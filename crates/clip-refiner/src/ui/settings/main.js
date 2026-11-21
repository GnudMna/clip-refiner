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
 * スナップショット受信時にフル再描画が必要かどうか
 *
 * @param {{ force?: boolean, externalSync?: boolean }} options
 * @param {boolean} wasEditing - 受信時にフォーム編集中だったか
 * @returns {boolean}
 */
function shouldFullRenderOnLoad(options, wasEditing) {
    if (options.force || options.externalSync) {
        return true;
    }
    if (clipRegisterPending && activeTab === 'clips') {
        return true;
    }
    if (!wasEditing) {
        return true;
    }
    return activeTab === 'favorites' || activeTab === 'shortcuts';
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

    if (shouldFullRenderOnLoad(opts, wasEditing)) {
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
 * @param {{ favoriteModes?: string[], shortcuts?: Array<{ binding: string, label: string }> }} payload
 */
window.updateFavoritesAndShortcuts = function updateFavoritesAndShortcuts(payload) {
    if (!state) {
        return;
    }
    state.favoriteModes = payload.favoriteModes || [];
    state.shortcuts = payload.shortcuts || [];
    if (activeTab === 'favorites') {
        refreshActivePanel();
    } else if (activeTab === 'shortcuts') {
        refreshActivePanel();
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
    if (event.key === 'Escape') {
        const dialog = document.getElementById('confirm-dialog');
        if (dialog && !dialog.hidden) {
            return;
        }
        if (isContentEditing()) {
            return;
        }
        closeWindow();
    }
});

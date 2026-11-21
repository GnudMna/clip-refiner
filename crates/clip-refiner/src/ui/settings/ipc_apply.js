// ======================================================================
// 設定画面 IPC 送信・パッチ debounce
// ======================================================================

/**
 * WebView IPC に生メッセージを送信する
 *
 * @param {string} message
 */
function postIpc(message) {
    window.ipc.postMessage(message);
}

/**
 * JSON エンベロープを IPC 送信する
 *
 * @param {Record<string, unknown>} envelope
 */
function postIpcEnvelope(envelope) {
    postIpc(JSON.stringify(envelope));
}

/**
 * 設定適用のフィードバックバナーを表示する
 *
 * @param {{ ok?: boolean, message?: string, field?: string }} [feedback]
 */
function showApplyFeedback(feedback) {
    const el = document.getElementById('apply-feedback');
    if (!el) {
        return;
    }
    clearTimeout(applyFeedbackTimer);
    const ok = feedback?.ok !== false;
    el.hidden = false;
    el.textContent = feedback?.message || (ok ? '設定を保存しました' : '設定の適用に失敗しました');
    el.className = 'apply-feedback apply-feedback--' + (ok ? 'ok' : 'error');
    applyFeedbackTimer = setTimeout(
        () => {
            el.hidden = true;
        },
        ok ? 2500 : 6000
    );
}

/**
 * 保留中パッチの無効な正規表現設定を除去する
 */
function stripInvalidPendingRegex() {
    if (pendingPatch.regex === undefined) {
        return;
    }
    const validation = validateRegexSettings(pendingPatch.regex);
    if (validation.valid) {
        return;
    }
    delete pendingPatch.regex;
    showApplyFeedback({
        ok: false,
        field: 'regex',
        message: validation.message,
    });
}

/**
 * デバウンス中の設定パッチを即時送信する
 */
function flushPendingApply() {
    if (applyTimer !== null) {
        clearTimeout(applyTimer);
        applyTimer = null;
    }
    stripInvalidPendingRegex();
    if (Object.keys(pendingPatch).length > 0) {
        postIpcEnvelope({ type: 'apply', patch: pendingPatch });
        pendingPatch = {};
    }
}

/** 保留パッチをフラッシュして設定ウィンドウを閉じる */
function closeWindow() {
    if (applyTimer !== null) {
        clearTimeout(applyTimer);
        applyTimer = null;
    }
    stripInvalidPendingRegex();
    const messages = SettingsSharedLogic.collectCloseWindowIpcMessages(pendingPatch);
    pendingPatch = {};
    for (const message of messages) {
        postIpcEnvelope(message);
    }
}

/** `config.toml` を OS の既定エディタで開く */
function openConfigFile() {
    postIpcEnvelope({ type: 'open-config' });
}

/**
 * `SettingsPatch.mergePatch` のエイリアス
 *
 * @param {Record<string, unknown>} target
 * @param {Record<string, unknown>} patch
 * @returns {Record<string, unknown>}
 */
function mergePatch(target, patch) {
    return SettingsPatch.mergePatch(target, patch);
}

/**
 * 設定パッチをデバウンスして IPC 送信する
 *
 * @param {Record<string, unknown>} patch
 */
function scheduleApply(patch) {
    mergePatch(pendingPatch, patch);
    clearTimeout(applyTimer);
    applyTimer = setTimeout(() => {
        stripInvalidPendingRegex();
        if (Object.keys(pendingPatch).length === 0) {
            applyTimer = null;
            return;
        }
        postIpcEnvelope({ type: 'apply', patch: pendingPatch });
        pendingPatch = {};
        applyTimer = null;
    }, 200);
}

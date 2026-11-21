// ======================================================================
// 設定画面 IPC 送信・パッチ debounce
// ======================================================================

import { ctx } from './ctx.js';
import { validateRegexSettings } from './regex_feedback.js';

import SettingsPatch from './settings_patch.js';
import SettingsSharedLogic from './shared_logic.js';

/**
 * WebView IPC に生メッセージを送信する
 *
 * @param {string} message
 */
export function postIpc(message) {
    window.ipc.postMessage(message);
}

/**
 * JSON エンベロープを IPC 送信する
 *
 * @param {Record<string, unknown>} envelope
 */
export function postIpcEnvelope(envelope) {
    postIpc(JSON.stringify(envelope));
}

/**
 * 設定適用のフィードバックバナーを表示する
 *
 * @param {{ ok?: boolean, message?: string, field?: string }} [feedback]
 */
export function showApplyFeedback(feedback) {
    const el = document.getElementById('apply-feedback');
    if (!el) {
        return;
    }
    clearTimeout(ctx.applyFeedbackTimer);
    const ok = feedback?.ok !== false;
    el.hidden = false;
    el.textContent = feedback?.message || (ok ? '設定を保存しました' : '設定の適用に失敗しました');
    el.className = 'apply-feedback apply-feedback--' + (ok ? 'ok' : 'error');
    ctx.applyFeedbackTimer = setTimeout(
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
    if (ctx.pendingPatch.regex === undefined) {
        return;
    }
    const validation = validateRegexSettings(ctx.pendingPatch.regex);
    if (validation.valid) {
        return;
    }
    delete ctx.pendingPatch.regex;
    showApplyFeedback({
        ok: false,
        field: 'regex',
        message: validation.message,
    });
}

/**
 * デバウンス中の設定パッチを即時送信する
 */
export function flushPendingApply() {
    if (ctx.applyTimer !== null) {
        clearTimeout(ctx.applyTimer);
        ctx.applyTimer = null;
    }
    stripInvalidPendingRegex();
    if (Object.keys(ctx.pendingPatch).length > 0) {
        postIpcEnvelope({ type: 'apply', patch: ctx.pendingPatch });
        ctx.pendingPatch = {};
    }
}

/** 保留パッチをフラッシュして設定ウィンドウを閉じる */
export function closeWindow() {
    if (ctx.applyTimer !== null) {
        clearTimeout(ctx.applyTimer);
        ctx.applyTimer = null;
    }
    stripInvalidPendingRegex();
    const messages = SettingsSharedLogic.collectCloseWindowIpcMessages(ctx.pendingPatch);
    ctx.pendingPatch = {};
    for (const message of messages) {
        postIpcEnvelope(message);
    }
}

/** `config.toml` を OS の既定エディタで開く */
export function openConfigFile() {
    postIpcEnvelope({ type: 'open-config' });
}

/**
 * 配布物に同梱する文書を OS の既定アプリで開く
 *
 * @param {'user-docs'|'third-party-notices'} target
 */
export function openDocument(target) {
    postIpcEnvelope({ type: 'open-document', target });
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
export function scheduleApply(patch) {
    mergePatch(ctx.pendingPatch, patch);
    clearTimeout(ctx.applyTimer);
    ctx.applyTimer = setTimeout(() => {
        stripInvalidPendingRegex();
        if (Object.keys(ctx.pendingPatch).length === 0) {
            ctx.applyTimer = null;
            return;
        }
        postIpcEnvelope({ type: 'apply', patch: ctx.pendingPatch });
        ctx.pendingPatch = {};
        ctx.applyTimer = null;
    }, 200);
}

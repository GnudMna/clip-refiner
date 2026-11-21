// ======================================================================
// 設定画面 正規表現プレビュー (DOM 連携)
// ======================================================================

import { ctx } from './ctx.js';
import { postIpcEnvelope } from './ipc_apply.js';
import { maxRegexPatternBytes } from './shared.js';

import RegexPreview from './regex_preview.js';

/**
 * 正規表現タブの DOM から現在の設定を読み取る
 *
 * @returns {import('./regex_preview').RegexSettings}
 */
export function readRegexFromDom() {
    return {
        pattern: document.getElementById('regex-pattern')?.value ?? ctx.state.regex.pattern,
        replacement:
            document.getElementById('regex-replacement')?.value ?? ctx.state.regex.replacement,
        caseInsensitive:
            document.getElementById('regex-ci')?.checked ?? ctx.state.regex.caseInsensitive,
        multiline: document.getElementById('regex-ml')?.checked ?? ctx.state.regex.multiline,
    };
}

/**
 * 正規表現設定を検証する
 *
 * @param {import('./regex_preview').RegexSettings} settings
 * @returns {import('./regex_preview').RegexValidationResult}
 */
export function validateRegexSettings(settings) {
    return RegexPreview.validateRegexSettings(
        settings,
        ctx.lastRegexPreview,
        maxRegexPatternBytes()
    );
}

/**
 * Rust 側に正規表現プレビューをリクエストする
 *
 * @param {import('./regex_preview').RegexSettings} settings
 */
export function requestRegexPreview(settings) {
    const requestId = ++ctx.regexPreviewRequestId;
    ctx.latestRegexPreviewId = requestId;
    ctx.pendingRegexPreviewRequest = {
        requestId,
        settings: RegexPreview.copyRegexSettings(settings),
    };
    postIpcEnvelope({
        type: 'regex-preview',
        requestId,
        settings: {
            pattern: settings.pattern,
            replacement: settings.replacement,
            caseInsensitive: settings.caseInsensitive,
            multiline: settings.multiline,
        },
    });

    const previewEl = document.getElementById('regex-preview-result');
    if (previewEl) {
        previewEl.textContent = '…';
        previewEl.classList.remove('regex-preview-result--error');
    }
}

/**
 * 正規表現プレビュー IPC 応答を処理する
 *
 * @param {import('./regex_preview').RegexPreviewPayload} payload
 */
export function handleRegexPreview(payload) {
    const statusEl = document.getElementById('regex-status');
    const previewEl = document.getElementById('regex-preview-result');
    const patternField = document.getElementById('regex-pattern')?.closest('.field');
    if (!statusEl || !previewEl) {
        return;
    }

    const result = RegexPreview.resolveRegexPreviewResponse({
        payload,
        latestRequestId: ctx.latestRegexPreviewId,
        pendingRequest: ctx.pendingRegexPreviewRequest,
        domSettings: readRegexFromDom(),
    });

    if (result.action === 'ignore') {
        return;
    }
    if (result.action === 'stale') {
        scheduleRegexFeedback();
        return;
    }

    ctx.lastRegexPreview = result.lastRegexPreview;
    const ui = result.ui;
    statusEl.textContent = ui.statusMessage;
    statusEl.className = ui.statusClass;

    if (patternField) {
        patternField.classList.toggle('field-invalid', ui.fieldInvalid);
    }

    previewEl.textContent = ui.previewText;
    previewEl.classList.toggle('regex-preview-result--error', ui.previewError);
}

/** DOM の正規表現設定でプレビューを即時更新する */
export function updateRegexFeedback() {
    requestRegexPreview(readRegexFromDom());
}

/** 正規表現プレビュー更新をデバウンスする */
export function scheduleRegexFeedback() {
    clearTimeout(ctx.regexFeedbackTimer);
    ctx.regexFeedbackTimer = setTimeout(updateRegexFeedback, 150);
}

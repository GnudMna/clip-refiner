// ======================================================================
// 設定画面 正規表現プレビュー (DOM 連携)
// ======================================================================

import { inputById } from '../../shared/dom.ts';
import { ctx } from '../ctx.ts';
import { postIpcEnvelope } from '../ipc_apply.ts';
import { readRegexFromDom } from './regex_ui.ts';

import RegexPreview from './regex_preview.ts';

import type { RegexPreviewPayload, RegexSettings } from './regex_preview.ts';

export { readRegexForApply } from './regex_ui.ts';

/**
 * 正規表現プレビュー用サンプル文字列を DOM から読み取る
 */
export function readRegexSampleFromDom(): string {
    return inputById('regex-preview-sample')?.value ?? ctx.state?.regexPreviewSample ?? '';
}

/**
 * Rust 側に正規表現プレビューをリクエストする
 */
function requestRegexPreview(settings: RegexSettings, sample: string): void {
    const requestId = ++ctx.regexPreviewRequestId;
    ctx.latestRegexPreviewId = requestId;
    ctx.pendingRegexPreviewRequest = {
        requestId,
        settings: RegexPreview.copyRegexSettings(settings),
        sample,
    };
    postIpcEnvelope({
        type: 'regex-preview',
        requestId,
        settings: RegexPreview.copyRegexSettings(settings),
        sample,
    });

    const previewEl = document.getElementById('regex-preview-result');
    if (previewEl) {
        previewEl.textContent = '…';
        previewEl.classList.remove('regex-preview-result--error');
    }
}

/**
 * 正規表現プレビュー IPC 応答を処理する
 */
export function handleRegexPreview(payload: RegexPreviewPayload): void {
    const statusEl = document.getElementById('regex-status');
    const previewEl = document.getElementById('regex-preview-result');
    if (!statusEl || !previewEl) {
        return;
    }

    const result = RegexPreview.resolveRegexPreviewResponse({
        payload,
        latestRequestId: ctx.latestRegexPreviewId,
        pendingRequest: ctx.pendingRegexPreviewRequest,
        domSettings: readRegexFromDom(),
        domSample: readRegexSampleFromDom(),
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

    document.querySelectorAll('.regex-step .field').forEach((field) => {
        field.classList.toggle('field-invalid', ui.fieldInvalid);
    });

    previewEl.textContent = ui.previewText;
    previewEl.classList.toggle('regex-preview-result--error', ui.previewError);
}

/** DOM の正規表現設定でプレビューを即時更新する */
export function updateRegexFeedback(): void {
    requestRegexPreview(readRegexFromDom(), readRegexSampleFromDom());
}

/** 正規表現プレビュー更新をデバウンスする */
export function scheduleRegexFeedback(): void {
    clearTimeout(ctx.regexFeedbackTimer ?? undefined);
    ctx.regexFeedbackTimer = setTimeout(updateRegexFeedback, 150);
}

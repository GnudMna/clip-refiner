// ======================================================================
// 正規表現プレビュー (純粋ロジック)
// ======================================================================

/**
 * 正規表現プレビュー IPC のクライアント側純粋ロジック
 *
 * @namespace RegexPreview
 */

/**
 * @typedef {Object} RegexSettings
 * @property {string} pattern - 正規表現パターン
 * @property {string} replacement - 置換文字列
 * @property {boolean} caseInsensitive - 大文字小文字を無視するか
 * @property {boolean} multiline - 複数行モードか
 */

/**
 * @typedef {Object} RegexPreviewCache
 * @property {RegexSettings|null} settings - 最後に検証した設定
 * @property {boolean} valid - Rust プレビュー応答の有効性
 * @property {string} message - ステータスメッセージ
 */

/**
 * @typedef {Object} RegexValidationResult
 * @property {boolean} valid - 適用可能か
 * @property {'empty'|'error'|'ok'|'pending'} status - 検証状態
 * @property {string} message - 表示用メッセージ
 */

/**
 * @typedef {Object} RegexPreviewPayload
 * @property {number} requestId - リクエスト ID
 * @property {boolean} valid - パターンが有効か
 * @property {string} message - ステータスメッセージ
 * @property {string|null} [result] - プレビュー結果テキスト
 */

/**
 * @typedef {Object} RegexPreviewUi
 * @property {string} statusMessage - ステータス欄のテキスト
 * @property {string} statusClass - ステータス欄の CSS クラス
 * @property {boolean} fieldInvalid - パターンフィールドを無効表示にするか
 * @property {string} previewText - プレビュー結果テキスト
 * @property {boolean} previewError - プレビュー結果をエラー表示にするか
 */

/**
 * @typedef {Object} RegexPreviewResolveResult
 * @property {'ignore'|'stale'|'apply'} action - 応答の処理方針
 * @property {RegexPreviewCache} [lastRegexPreview] - キャッシュ更新 (action が `apply` のとき)
 * @property {RegexPreviewUi} [ui] - UI 更新用フィールド (action が `apply` のとき)
 */

var RegexPreview = (function () {
    'use strict';

    /**
     * 正規表現設定 2 件が同一かどうか
     *
     * @param {RegexSettings|null|undefined} a
     * @param {RegexSettings|null|undefined} b
     * @returns {boolean}
     */
    function regexSettingsEqual(a, b) {
        if (!a || !b) {
            return false;
        }
        return (
            a.pattern === b.pattern &&
            a.replacement === b.replacement &&
            a.caseInsensitive === b.caseInsensitive &&
            a.multiline === b.multiline
        );
    }

    /**
     * 設定オブジェクトの浅いコピー
     *
     * @param {RegexSettings} settings
     * @returns {RegexSettings}
     */
    function copyRegexSettings(settings) {
        return {
            pattern: settings.pattern,
            replacement: settings.replacement,
            caseInsensitive: settings.caseInsensitive,
            multiline: settings.multiline,
        };
    }

    /**
     * クライアント側の即時チェック
     *
     * バイト長のみ検証する。構文の妥当性は Rust プレビュー応答を参照する
     *
     * @param {RegexSettings} settings - 検証対象
     * @param {RegexPreviewCache} lastRegexPreview - 直近のプレビューキャッシュ
     * @param {number} maxPatternBytes - パターンの最大バイト数
     * @returns {RegexValidationResult}
     */
    function validateRegexSettings(settings, lastRegexPreview, maxPatternBytes) {
        const pattern = settings.pattern ?? '';
        if (pattern.length === 0) {
            return {
                valid: true,
                status: 'empty',
                message: 'パターンが空です (マッチしません)',
            };
        }
        if (new TextEncoder().encode(pattern).length > maxPatternBytes) {
            return {
                valid: false,
                status: 'error',
                message: 'パターンが長すぎます (最大 ' + maxPatternBytes + ' バイト)',
            };
        }
        if (regexSettingsEqual(settings, lastRegexPreview.settings)) {
            return {
                valid: lastRegexPreview.valid,
                status: lastRegexPreview.valid ? 'ok' : 'error',
                message: lastRegexPreview.message,
            };
        }
        return {
            valid: true,
            status: 'pending',
            message: '',
        };
    }

    /**
     * プレビュー応答から UI 更新用フィールドを組み立てる
     *
     * @param {RegexPreviewPayload} payload - IPC 応答
     * @param {RegexSettings} domSettings - 現在の DOM 上の設定
     * @returns {RegexPreviewUi}
     */
    function buildPreviewUi(payload, domSettings) {
        const pattern = domSettings.pattern ?? '';
        let status = 'ok';
        if (!payload.valid) {
            status = 'error';
        } else if (!pattern) {
            status = 'empty';
        }
        return {
            statusMessage: payload.message,
            statusClass: 'regex-status regex-status--' + status,
            fieldInvalid: status === 'error',
            previewText: payload.result ?? '—',
            previewError: !payload.valid && pattern.length > 0,
        };
    }

    /**
     * プレビュー IPC 応答を解決する
     *
     * DOM がリクエスト送信時から変わっている場合は stale としてキャッシュ/UI を更新しない
     *
     * @param {Object} options
     * @param {RegexPreviewPayload} options.payload - IPC 応答
     * @param {number} options.latestRequestId - 最新リクエスト ID
     * @param {{ requestId: number, settings: RegexSettings }|null} options.pendingRequest - 送信中リクエスト
     * @param {RegexSettings} options.domSettings - 現在の DOM 上の設定
     * @returns {RegexPreviewResolveResult}
     */
    function resolveRegexPreviewResponse({
        payload,
        latestRequestId,
        pendingRequest,
        domSettings,
    }) {
        if (payload.requestId !== latestRequestId) {
            return { action: 'ignore' };
        }
        if (!pendingRequest || pendingRequest.requestId !== payload.requestId) {
            return { action: 'ignore' };
        }
        if (!regexSettingsEqual(domSettings, pendingRequest.settings)) {
            return { action: 'stale' };
        }

        const lastRegexPreview = {
            settings: copyRegexSettings(pendingRequest.settings),
            valid: payload.valid,
            message: payload.message,
        };
        return {
            action: 'apply',
            lastRegexPreview,
            ui: buildPreviewUi(payload, domSettings),
        };
    }

    return {
        regexSettingsEqual,
        copyRegexSettings,
        validateRegexSettings,
        buildPreviewUi,
        resolveRegexPreviewResponse,
    };
})();

if (typeof module !== 'undefined' && module.exports) {
    module.exports = RegexPreview;
}

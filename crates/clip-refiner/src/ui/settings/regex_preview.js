// ======================================================================
// 正規表現プレビュー (純粋ロジック)
// ======================================================================

/**
 * 正規表現プレビュー IPC のクライアント側純粋ロジック
 *
 * @namespace RegexPreview
 */

/**
 * @typedef {Object} RegexStep
 * @property {string} pattern - 正規表現パターン
 * @property {string} replacement - 置換文字列
 * @property {boolean} caseInsensitive - 大文字小文字を無視するか
 * @property {boolean} multiline - 複数行モードか
 */

/**
 * @typedef {Object} RegexPreset
 * @property {string} id - プリセット ID
 * @property {string} name - 表示名
 * @property {RegexStep[]} steps - ステップ列
 */

/**
 * @typedef {Object} RegexSettings
 * @property {string} activePresetId - 有効プリセット ID
 * @property {RegexPreset[]} presets - プリセット一覧
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
 * @typedef {Object} RegexPreviewPending
 * @property {number} requestId - リクエスト ID
 * @property {RegexSettings} settings - 送信時の正規表現設定
 * @property {string} sample - 送信時のサンプル文字列
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
        return JSON.stringify(a) === JSON.stringify(b);
    }

    /**
     * ステップの浅いコピー
     *
     * @param {RegexStep} step
     * @returns {RegexStep}
     */
    function copyStep(step) {
        return {
            pattern: step.pattern ?? '',
            replacement: step.replacement ?? '',
            caseInsensitive: !!step.caseInsensitive,
            multiline: !!step.multiline,
        };
    }

    /**
     * 設定オブジェクトのコピー
     *
     * @param {RegexSettings} settings
     * @returns {RegexSettings}
     */
    function copyRegexSettings(settings) {
        return {
            activePresetId: settings.activePresetId ?? '',
            presets: (settings.presets || []).map(function (preset) {
                return {
                    id: preset.id ?? '',
                    name: preset.name ?? '',
                    steps: (preset.steps || []).map(copyStep),
                };
            }),
        };
    }

    /**
     * 有効 (または先頭) プリセットのステップを返す
     *
     * @param {RegexSettings} settings
     * @returns {RegexStep[]}
     */
    function activeSteps(settings) {
        const presets = settings.presets || [];
        if (presets.length === 0) {
            return [];
        }
        const active =
            presets.find(function (preset) {
                return preset.id === settings.activePresetId;
            }) || presets[0];
        return active.steps || [];
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
        const encoder = new TextEncoder();
        const presets = settings.presets || [];
        for (let p = 0; p < presets.length; p += 1) {
            const preset = presets[p];
            const steps = preset.steps || [];
            for (let s = 0; s < steps.length; s += 1) {
                const pattern = steps[s].pattern ?? '';
                if (encoder.encode(pattern).length > maxPatternBytes) {
                    return {
                        valid: false,
                        status: 'error',
                        message:
                            'プリセット「' +
                            (preset.name || '') +
                            '」のステップ ' +
                            (s + 1) +
                            ' : パターンが長すぎます (最大 ' +
                            maxPatternBytes +
                            ' バイト)',
                    };
                }
            }
        }

        const steps = activeSteps(settings);
        const allEmpty = steps.every(function (step) {
            return !(step.pattern ?? '');
        });
        if (allEmpty) {
            return {
                valid: true,
                status: 'empty',
                message: 'パターンが空です (マッチしません)',
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
        const steps = activeSteps(domSettings);
        const allEmpty = steps.every(function (step) {
            return !(step.pattern ?? '');
        });
        let status = 'ok';
        if (!payload.valid) {
            status = 'error';
        } else if (allEmpty) {
            status = 'empty';
        }
        return {
            statusMessage: payload.message,
            statusClass: 'regex-status regex-status--' + status,
            fieldInvalid: status === 'error',
            previewText: payload.result ?? '—',
            previewError: !payload.valid && !allEmpty,
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
     * @param {RegexPreviewPending|null} options.pendingRequest - 送信中リクエスト
     * @param {RegexSettings} options.domSettings - 現在の DOM 上の設定
     * @param {string} options.domSample - 現在の DOM 上のサンプル文字列
     * @returns {RegexPreviewResolveResult}
     */
    function resolveRegexPreviewResponse({
        payload,
        latestRequestId,
        pendingRequest,
        domSettings,
        domSample,
    }) {
        if (payload.requestId !== latestRequestId) {
            return { action: 'ignore' };
        }
        if (!pendingRequest || pendingRequest.requestId !== payload.requestId) {
            return { action: 'ignore' };
        }
        if (
            !regexSettingsEqual(domSettings, pendingRequest.settings) ||
            (domSample ?? '') !== (pendingRequest.sample ?? '')
        ) {
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
        copyStep,
        activeSteps,
        validateRegexSettings,
        buildPreviewUi,
        resolveRegexPreviewResponse,
    };
})();

if (typeof module !== 'undefined' && module.exports) {
    module.exports = RegexPreview;
}

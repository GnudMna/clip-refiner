    // ======================================================================
    // 正規表現プレビュー (純粋ロジック)
    // ======================================================================
    var RegexPreview = (function () {
        'use strict';

        // 正規表現設定 2 件が同一かどうか
        function regexSettingsEqual(a, b) {
            if (!a || !b) {
                return false;
            }
            return a.pattern === b.pattern
                && a.replacement === b.replacement
                && a.caseInsensitive === b.caseInsensitive
                && a.multiline === b.multiline;
        }

        // 設定オブジェクトの浅いコピー
        function copyRegexSettings(settings) {
            return {
                pattern: settings.pattern,
                replacement: settings.replacement,
                caseInsensitive: settings.caseInsensitive,
                multiline: settings.multiline,
            };
        }

        // クライアント側の即時チェック (バイト長のみ。構文は Rust プレビュー応答を参照)
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

        // プレビュー応答から UI 更新用フィールドを組み立てる
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

        // プレビュー IPC 応答を解決する
        //
        // DOM がリクエスト送信時から変わっている場合は stale としてキャッシュ/UI を更新しない
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

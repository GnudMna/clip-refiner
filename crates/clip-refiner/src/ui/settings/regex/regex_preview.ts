// ======================================================================
// 正規表現プレビュー (純粋ロジック)
// ======================================================================

/**
 * 正規表現プレビュー IPC のクライアント側純粋ロジック
 */

export interface RegexStep {
    /** 正規表現パターン */
    pattern: string;
    /** 置換文字列 */
    replacement: string;
    /** 大文字小文字を無視するか */
    caseInsensitive: boolean;
    /** 複数行モードか */
    multiline: boolean;
}

interface RegexPreset {
    /** プリセット ID */
    id: string;
    /** 表示名 */
    name: string;
    /** ステップ列 */
    steps: RegexStep[];
}

export interface RegexSettings {
    /** 有効プリセット ID */
    activePresetId: string;
    /** プリセット一覧 */
    presets: RegexPreset[];
}

export interface RegexPreviewCache {
    /** 最後に検証した設定 */
    settings: RegexSettings | null;
    /** Rust プレビュー応答の有効性 */
    valid: boolean;
    /** ステータスメッセージ */
    message: string;
}

export interface RegexValidationResult {
    /** 適用可能か */
    valid: boolean;
    /** 検証状態 */
    status: 'empty' | 'error' | 'ok' | 'pending';
    /** 表示用メッセージ */
    message: string;
}

export interface RegexPreviewPayload {
    /** リクエスト ID */
    requestId: number;
    /** パターンが有効か */
    valid: boolean;
    /** ステータスメッセージ */
    message: string;
    /** プレビュー結果テキスト */
    result?: string | null;
}

interface RegexPreviewUi {
    /** ステータス欄のテキスト */
    statusMessage: string;
    /** ステータス欄の CSS クラス */
    statusClass: string;
    /** パターンフィールドを無効表示にするか */
    fieldInvalid: boolean;
    /** プレビュー結果テキスト */
    previewText: string;
    /** プレビュー結果をエラー表示にするか */
    previewError: boolean;
}

export interface RegexPreviewPending {
    /** リクエスト ID */
    requestId: number;
    /** 送信時の正規表現設定 */
    settings: RegexSettings;
    /** 送信時のサンプル文字列 */
    sample: string;
}

/**
 * 応答の処理方針。`apply` のときだけキャッシュ更新 (`lastRegexPreview`) と UI 更新用フィールド (`ui`) を持つ
 */
type RegexPreviewResolveResult =
    | { action: 'ignore' }
    | { action: 'stale' }
    | { action: 'apply'; lastRegexPreview: RegexPreviewCache; ui: RegexPreviewUi };

const RegexPreview = (function () {
    'use strict';

    /**
     * 正規表現設定 2 件が同一かどうか
     */
    function regexSettingsEqual(
        a: RegexSettings | null | undefined,
        b: RegexSettings | null | undefined
    ): boolean {
        if (!a || !b) {
            return false;
        }
        return JSON.stringify(a) === JSON.stringify(b);
    }

    /**
     * ステップの浅いコピー
     */
    function copyStep(step: RegexStep): RegexStep {
        return {
            pattern: step.pattern ?? '',
            replacement: step.replacement ?? '',
            caseInsensitive: !!step.caseInsensitive,
            multiline: !!step.multiline,
        };
    }

    /**
     * 設定オブジェクトのコピー
     */
    function copyRegexSettings(settings: RegexSettings): RegexSettings {
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
     */
    function activeSteps(settings: RegexSettings): RegexStep[] {
        const presets = settings.presets || [];
        if (presets.length === 0) {
            return [];
        }
        const active =
            presets.find(function (preset) {
                return preset.id === settings.activePresetId;
            }) || presets[0];
        return active?.steps || [];
    }

    /**
     * クライアント側の即時チェック
     *
     * バイト長のみ検証する。構文の妥当性は Rust プレビュー応答を参照する
     *
     * @param settings - 検証対象
     * @param lastRegexPreview - 直近のプレビューキャッシュ
     * @param maxPatternBytes - パターンの最大バイト数
     */
    function validateRegexSettings(
        settings: RegexSettings,
        lastRegexPreview: RegexPreviewCache,
        maxPatternBytes: number
    ): RegexValidationResult {
        const encoder = new TextEncoder();
        const presets = settings.presets || [];
        for (let p = 0; p < presets.length; p += 1) {
            const preset = presets[p];
            if (!preset) {
                continue;
            }
            const steps = preset.steps || [];
            for (let s = 0; s < steps.length; s += 1) {
                const pattern = steps[s]?.pattern ?? '';
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
     * @param payload - IPC 応答
     * @param domSettings - 現在の DOM 上の設定
     */
    function buildPreviewUi(
        payload: RegexPreviewPayload,
        domSettings: RegexSettings
    ): RegexPreviewUi {
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
     */
    function resolveRegexPreviewResponse({
        payload,
        latestRequestId,
        pendingRequest,
        domSettings,
        domSample,
    }: {
        /** IPC 応答 */
        payload: RegexPreviewPayload;
        /** 最新リクエスト ID */
        latestRequestId: number;
        /** 送信中リクエスト */
        pendingRequest: RegexPreviewPending | null;
        /** 現在の DOM 上の設定 */
        domSettings: RegexSettings;
        /** 現在の DOM 上のサンプル文字列 */
        domSample: string;
    }): RegexPreviewResolveResult {
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

export default RegexPreview;

// ======================================================================
// 設定画面 サーバー提供の制限値
// ======================================================================

import { ctx } from './ctx.ts';

import type { SettingsLimits } from '../ipc.ts';

/**
 * サーバーから渡された制限値オブジェクトを返す
 */
function limits(): Partial<SettingsLimits> {
    return ctx.state?.limits ?? {};
}

/**
 * 履歴保持件数の min/max
 */
export function historyLimitBounds(): { min: number; max: number } {
    const l = limits();
    return {
        min: l.historyMin ?? 1,
        max: l.historyMax ?? 100,
    };
}

/**
 * 連続記録保持件数の min/max
 */
export function recordLimitBounds(): { min: number; max: number } {
    const l = limits();
    return {
        min: l.continuousRecordMin ?? 1,
        max: l.continuousRecordMax ?? 100,
    };
}

/**
 * ポーリング間隔 (ミリ秒) の min/max
 */
export function intervalMsBounds(): { min: number; max: number } {
    const l = limits();
    return {
        min: l.intervalMinMs ?? 100,
        max: l.intervalMaxMs ?? 60_000,
    };
}

/**
 * セレクター WebView アイドル破棄分数の min/max
 */
export function selectorWebviewIdleMinutesBounds(): { min: number; max: number } {
    const l = limits();
    return {
        min: l.selectorWebviewIdleMinMinutes ?? 1,
        max: l.selectorWebviewIdleMaxMinutes ?? 1440,
    };
}

/** 加工パイプラインの最大段数 */
export function maxPipelineLength(): number {
    return limits().maxPipelineLength ?? 10;
}

/** お気に入りモードの最大件数 */
export function maxFavoriteModes(): number {
    return limits().maxFavoriteModes ?? 20;
}

/** 登録クリップの最大件数 */
export function maxRegisteredClips(): number {
    return limits().maxRegisteredClips ?? 100;
}

/** 登録クリップのラベルの最大文字数 */
export function maxRegisteredClipLabelChars(): number {
    return limits().maxRegisteredClipLabelChars ?? 64;
}

/** 正規表現パターンの最大バイト数 */
export function maxRegexPatternBytes(): number {
    return limits().maxRegexPatternBytes ?? 8 * 1024;
}

/** 正規表現プリセットの最大件数 */
export function maxRegexPresets(): number {
    return limits().maxRegexPresets ?? 20;
}

/** 正規表現プリセットあたりの最大ステップ数 */
export function maxRegexStepsPerPreset(): number {
    return limits().maxRegexStepsPerPreset ?? 10;
}

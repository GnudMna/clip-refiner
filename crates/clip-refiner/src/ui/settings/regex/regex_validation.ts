// ======================================================================
// 設定画面 正規表現設定の検証
// ======================================================================

import { ctx } from '../ctx.ts';
import { maxRegexPatternBytes } from '../limits.ts';

import RegexPreview from './regex_preview.ts';

import type { RegexSettings, RegexValidationResult } from './regex_preview.ts';

/**
 * 正規表現設定を検証する
 */
export function validateRegexSettings(settings: RegexSettings): RegexValidationResult {
    return RegexPreview.validateRegexSettings(
        settings,
        ctx.lastRegexPreview,
        maxRegexPatternBytes()
    );
}

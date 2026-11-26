/**
 * `RegexPreview` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';
import { test } from 'node:test';

import RegexPreview from '../../src/ui/settings/regex/regex_preview.ts';

import type {
    RegexPreviewPayload,
    RegexPreviewPending,
    RegexSettings,
    RegexStep,
} from '../../src/ui/settings/regex/regex_preview.ts';

const MAX_PATTERN_BYTES = 8 * 1024;
const SAMPLE = 'Hello world 123';

function step(pattern: string): RegexStep {
    return { pattern, replacement: '', caseInsensitive: false, multiline: false };
}

function settingsWithPattern(pattern: string): RegexSettings {
    return {
        activePresetId: 'default',
        presets: [
            {
                id: 'default',
                name: 'デフォルト',
                steps: [step(pattern)],
            },
        ],
    };
}

const validSettings: RegexSettings = settingsWithPattern('\\d+');

const invalidSettings: RegexSettings = settingsWithPattern('[\\d+');

/**
 * プレビュー IPC 応答のテスト用ペイロードを組み立てる
 */
function previewPayload(
    requestId: number,
    valid: boolean,
    message: string,
    result: string | null
): RegexPreviewPayload {
    return { requestId, valid, message, result };
}

function pendingRequest(
    requestId: number,
    settings: RegexSettings,
    sample: string = SAMPLE
): RegexPreviewPending {
    return { requestId, settings, sample };
}

/** 応答到着時に DOM がリクエスト送信時から変わっている場合は stale となること */
test('stale_response_when_dom_changed', () => {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 1,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: invalidSettings,
        domSample: SAMPLE,
    });

    assert.equal(result.action, 'stale');
    assert.deepEqual(result, { action: 'stale' });
});

/** サンプル文字列が変わっている場合は stale となること */
test('stale_response_when_sample_changed', () => {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 1,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: validSettings,
        domSample: 'other sample',
    });

    assert.equal(result.action, 'stale');
});

/** DOM が一致する場合のみキャッシュと UI が更新されること */
test('apply_response_when_dom_matches_request', () => {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(2, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 2,
        pendingRequest: pendingRequest(2, validSettings),
        domSettings: validSettings,
        domSample: SAMPLE,
    });

    assert.equal(result.action, 'apply');
    assert.deepEqual(result.lastRegexPreview.settings, validSettings);
    assert.equal(result.lastRegexPreview.valid, true);
    assert.equal(result.ui.previewText, 'Hello world #');
    assert.equal(result.ui.fieldInvalid, false);
});

/** 古い requestId の応答は無視されること */
test('ignore_outdated_response', () => {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'x'),
        latestRequestId: 2,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: validSettings,
        domSample: SAMPLE,
    });

    assert.equal(result.action, 'ignore');
});

/** stale 応答後も validateRegexSettings が pending を返すこと (誤った valid:true を使わない) */
test('validate_pending_after_stale_response', () => {
    const stale = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'x'),
        latestRequestId: 1,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: invalidSettings,
        domSample: SAMPLE,
    });
    assert.equal(stale.action, 'stale');

    const validation = RegexPreview.validateRegexSettings(
        invalidSettings,
        { settings: null, valid: true, message: '' },
        MAX_PATTERN_BYTES
    );
    assert.equal(validation.status, 'pending');
    assert.equal(validation.valid, true);
});

/** 正常応答後はキャッシュが validateRegexSettings に使えること */
test('validate_uses_cache_after_apply', () => {
    const applied = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(3, false, 'パターンが無効です: ...', null),
        latestRequestId: 3,
        pendingRequest: pendingRequest(3, invalidSettings),
        domSettings: invalidSettings,
        domSample: SAMPLE,
    });
    assert.equal(applied.action, 'apply');

    const validation = RegexPreview.validateRegexSettings(
        invalidSettings,
        applied.lastRegexPreview,
        MAX_PATTERN_BYTES
    );
    assert.equal(validation.valid, false);
    assert.equal(validation.status, 'error');
});

/** 空パターンの UI 状態 */
test('build_preview_ui_empty_pattern', () => {
    const ui = RegexPreview.buildPreviewUi(
        previewPayload(1, true, 'パターンが空です (マッチしません)', 'sample'),
        settingsWithPattern('')
    );
    assert.equal(ui.statusClass, 'regex-status regex-status--empty');
    assert.equal(ui.fieldInvalid, false);
});

/** タイムアウト応答をエラー表示として組み立てられること */
test('build_preview_ui_timeout_message', () => {
    const ui = RegexPreview.buildPreviewUi(
        previewPayload(1, false, '正規表現の実行がタイムアウトしました', null),
        settingsWithPattern('.+')
    );
    assert.equal(ui.statusClass, 'regex-status regex-status--error');
    assert.equal(ui.fieldInvalid, true);
    assert.equal(ui.previewError, true);
    assert.equal(ui.statusMessage, '正規表現の実行がタイムアウトしました');
    assert.equal(ui.previewText, '—');
});

/** regexSettingsEqual の基本動作 */
test('regex_settings_equal', () => {
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, validSettings), true);
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, settingsWithPattern('x')), false);
});

/** 片方でも設定が無ければ一致とみなさないこと */
test('regex_settings_equal_without_settings', () => {
    assert.equal(RegexPreview.regexSettingsEqual(null, validSettings), false);
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, undefined), false);
    assert.equal(RegexPreview.regexSettingsEqual(null, null), false);
});

/** コピーした設定を変更しても元の設定に影響しないこと */
test('copy_regex_settings_is_independent', () => {
    const original = settingsWithPattern('a');
    const copy = RegexPreview.copyRegexSettings(original);
    assert.deepEqual(copy, original);

    for (const preset of copy.presets) {
        preset.name = '変更後';
        for (const step of preset.steps) {
            step.pattern = '変更後';
        }
    }
    assert.deepEqual(original, settingsWithPattern('a'));
});

/** 欠けた項目を既定値で補ってコピーすること */
test('copy_regex_settings_fills_missing_fields', () => {
    assert.deepEqual(RegexPreview.copyRegexSettings({} as RegexSettings), {
        activePresetId: '',
        presets: [],
    });
    assert.deepEqual(
        RegexPreview.copyRegexSettings({
            activePresetId: 'a',
            presets: [{}, { id: 'b', name: 'B', steps: [{}] }],
        } as RegexSettings),
        {
            activePresetId: 'a',
            presets: [
                { id: '', name: '', steps: [] },
                { id: 'b', name: 'B', steps: [step('')] },
            ],
        }
    );
});

/** 有効プリセットのステップを返し、見つからなければ先頭のプリセットを使うこと */
test('active_steps', () => {
    const presets = [
        { id: 'first', name: '', steps: [step('a')] },
        { id: 'second', name: '', steps: [step('b')] },
    ];

    assert.deepEqual(RegexPreview.activeSteps({ activePresetId: 'second', presets }), [step('b')]);
    assert.deepEqual(RegexPreview.activeSteps({ activePresetId: 'missing', presets }), [step('a')]);
    assert.deepEqual(RegexPreview.activeSteps({ activePresetId: 'first', presets: [] }), []);
    assert.deepEqual(RegexPreview.activeSteps({} as RegexSettings), []);
});

/** パターンの長さを文字数ではなくバイト数で判定し、超えたプリセットとステップを示すこと */
test('validate_rejects_too_long_pattern', () => {
    const cache = { settings: null, valid: true, message: '' };
    const settings: RegexSettings = {
        activePresetId: 'short',
        presets: [
            { id: 'short', name: '短い', steps: [step('a')] },
            // 「あ」は UTF-8 で 3 バイト
            { id: 'long', name: '長い', steps: [step('a'), step('ああ')] },
        ],
    };

    assert.deepEqual(RegexPreview.validateRegexSettings(settings, cache, 5), {
        valid: false,
        status: 'error',
        message: 'プリセット「長い」のステップ 2 : パターンが長すぎます (最大 5 バイト)',
    });
    // 上限ちょうどは受け付ける
    assert.equal(RegexPreview.validateRegexSettings(settings, cache, 6).valid, true);
});

/** 全ステップのパターンが空なら empty となること */
test('validate_empty_patterns', () => {
    const cache = { settings: null, valid: true, message: '' };
    const expected = {
        valid: true,
        status: 'empty',
        message: 'パターンが空です (マッチしません)',
    };
    for (const settings of [settingsWithPattern(''), {} as RegexSettings]) {
        assert.deepEqual(
            RegexPreview.validateRegexSettings(settings, cache, MAX_PATTERN_BYTES),
            expected
        );
    }
});

/** 有効なキャッシュがあれば ok とそのメッセージを返すこと */
test('validate_uses_valid_cache', () => {
    assert.deepEqual(
        RegexPreview.validateRegexSettings(
            validSettings,
            { settings: validSettings, valid: true, message: 'パターンは有効です' },
            MAX_PATTERN_BYTES
        ),
        { valid: true, status: 'ok', message: 'パターンは有効です' }
    );
});

/** 送信中のリクエストが無い、または応答と対応しない場合は無視されること */
test('ignore_response_without_matching_request', () => {
    for (const pending of [null, pendingRequest(1, validSettings)]) {
        const result = RegexPreview.resolveRegexPreviewResponse({
            payload: previewPayload(2, true, 'パターンは有効です', 'x'),
            latestRequestId: 2,
            pendingRequest: pending,
            domSettings: validSettings,
            domSample: SAMPLE,
        });
        assert.deepEqual(result, { action: 'ignore' });
    }
});

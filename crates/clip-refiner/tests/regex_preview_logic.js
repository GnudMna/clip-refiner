/**
 * `RegexPreview` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';

import RegexPreview from '../src/ui/settings/regex_preview.js';

const MAX_PATTERN_BYTES = 8 * 1024;
const SAMPLE = 'Hello world 123';

/**
 * @param {string} pattern
 * @returns {import('../src/ui/settings/regex_preview').RegexSettings}
 */
function settingsWithPattern(pattern) {
    return {
        activePresetId: 'default',
        presets: [
            {
                id: 'default',
                name: 'デフォルト',
                steps: [
                    {
                        pattern,
                        replacement: '',
                        caseInsensitive: false,
                        multiline: false,
                    },
                ],
            },
        ],
    };
}

/** @type {import('../src/ui/settings/regex_preview').RegexSettings} */
const validSettings = settingsWithPattern('\\d+');

/** @type {import('../src/ui/settings/regex_preview').RegexSettings} */
const invalidSettings = settingsWithPattern('[\\d+');

/**
 * プレビュー IPC 応答のテスト用ペイロードを組み立てる
 *
 * @param {number} requestId
 * @param {boolean} valid
 * @param {string} message
 * @param {string|null} result
 * @returns {import('../src/ui/settings/regex_preview').RegexPreviewPayload}
 */
function previewPayload(requestId, valid, message, result) {
    return { requestId, valid, message, result };
}

/**
 * @param {number} requestId
 * @param {import('../src/ui/settings/regex_preview').RegexSettings} settings
 * @param {string} [sample]
 * @returns {import('../src/ui/settings/regex_preview').RegexPreviewPending}
 */
function pendingRequest(requestId, settings, sample = SAMPLE) {
    return { requestId, settings, sample };
}

/** 応答到着時に DOM がリクエスト送信時から変わっている場合は stale となること */
function test_stale_response_when_dom_changed() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 1,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: invalidSettings,
        domSample: SAMPLE,
    });

    assert.equal(result.action, 'stale');
    assert.equal(result.lastRegexPreview, undefined);
}

/** サンプル文字列が変わっている場合は stale となること */
function test_stale_response_when_sample_changed() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 1,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: validSettings,
        domSample: 'other sample',
    });

    assert.equal(result.action, 'stale');
}

/** DOM が一致する場合のみキャッシュと UI が更新されること */
function test_apply_response_when_dom_matches_request() {
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
}

/** 古い requestId の応答は無視されること */
function test_ignore_outdated_response() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'x'),
        latestRequestId: 2,
        pendingRequest: pendingRequest(1, validSettings),
        domSettings: validSettings,
        domSample: SAMPLE,
    });

    assert.equal(result.action, 'ignore');
}

/** stale 応答後も validateRegexSettings が pending を返すこと (誤った valid:true を使わない) */
function test_validate_pending_after_stale_response() {
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
}

/** 正常応答後はキャッシュが validateRegexSettings に使えること */
function test_validate_uses_cache_after_apply() {
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
}

/** 空パターンの UI 状態 */
function test_build_preview_ui_empty_pattern() {
    const ui = RegexPreview.buildPreviewUi(
        previewPayload(1, true, 'パターンが空です (マッチしません)', 'sample'),
        settingsWithPattern('')
    );
    assert.equal(ui.statusClass, 'regex-status regex-status--empty');
    assert.equal(ui.fieldInvalid, false);
}

/** タイムアウト応答をエラー表示として組み立てられること */
function test_build_preview_ui_timeout_message() {
    const ui = RegexPreview.buildPreviewUi(
        previewPayload(1, false, '正規表現の実行がタイムアウトしました', null),
        settingsWithPattern('.+')
    );
    assert.equal(ui.statusClass, 'regex-status regex-status--error');
    assert.equal(ui.fieldInvalid, true);
    assert.equal(ui.previewError, true);
    assert.equal(ui.statusMessage, '正規表現の実行がタイムアウトしました');
    assert.equal(ui.previewText, '—');
}

/** regexSettingsEqual の基本動作 */
function test_regex_settings_equal() {
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, validSettings), true);
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, settingsWithPattern('x')), false);
}

const tests = [
    test_stale_response_when_dom_changed,
    test_stale_response_when_sample_changed,
    test_apply_response_when_dom_matches_request,
    test_ignore_outdated_response,
    test_validate_pending_after_stale_response,
    test_validate_uses_cache_after_apply,
    test_build_preview_ui_empty_pattern,
    test_build_preview_ui_timeout_message,
    test_regex_settings_equal,
];

for (const run of tests) {
    run();
}

console.log(`regex_preview_logic: ${tests.length} tests passed`);

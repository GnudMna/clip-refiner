'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');

const RegexPreview = require(path.join(
    __dirname,
    '../src/ui/settings/regex_preview.js'
));

const MAX_PATTERN_BYTES = 8 * 1024;

const validSettings = {
    pattern: '\\d+',
    replacement: '',
    caseInsensitive: false,
    multiline: false,
};

const invalidSettings = {
    pattern: '[\\d+',
    replacement: '',
    caseInsensitive: false,
    multiline: false,
};

function previewPayload(requestId, valid, message, result) {
    return { requestId, valid, message, result };
}

// 応答到着時に DOM がリクエスト送信時から変わっている場合は stale となること
function test_stale_response_when_dom_changed() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 1,
        pendingRequest: { requestId: 1, settings: validSettings },
        domSettings: invalidSettings,
    });

    assert.equal(result.action, 'stale');
    assert.equal(result.lastRegexPreview, undefined);
}

// DOM が一致する場合のみキャッシュと UI が更新されること
function test_apply_response_when_dom_matches_request() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(2, true, 'パターンは有効です', 'Hello world #'),
        latestRequestId: 2,
        pendingRequest: { requestId: 2, settings: validSettings },
        domSettings: validSettings,
    });

    assert.equal(result.action, 'apply');
    assert.deepEqual(result.lastRegexPreview.settings, validSettings);
    assert.equal(result.lastRegexPreview.valid, true);
    assert.equal(result.ui.previewText, 'Hello world #');
    assert.equal(result.ui.fieldInvalid, false);
}

// 古い requestId の応答は無視されること
function test_ignore_outdated_response() {
    const result = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'x'),
        latestRequestId: 2,
        pendingRequest: { requestId: 1, settings: validSettings },
        domSettings: validSettings,
    });

    assert.equal(result.action, 'ignore');
}

// stale 応答後も validateRegexSettings が pending を返すこと (誤った valid:true を使わない)
function test_validate_pending_after_stale_response() {
    const stale = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(1, true, 'パターンは有効です', 'x'),
        latestRequestId: 1,
        pendingRequest: { requestId: 1, settings: validSettings },
        domSettings: invalidSettings,
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

// 正常応答後はキャッシュが validateRegexSettings に使えること
function test_validate_uses_cache_after_apply() {
    const applied = RegexPreview.resolveRegexPreviewResponse({
        payload: previewPayload(3, false, 'パターンが無効です: ...', null),
        latestRequestId: 3,
        pendingRequest: { requestId: 3, settings: invalidSettings },
        domSettings: invalidSettings,
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

// 空パターンの UI 状態
function test_build_preview_ui_empty_pattern() {
    const ui = RegexPreview.buildPreviewUi(
        previewPayload(1, true, 'パターンが空です (マッチしません)', 'sample'),
        { pattern: '', replacement: '', caseInsensitive: false, multiline: false }
    );
    assert.equal(ui.statusClass, 'regex-status regex-status--empty');
    assert.equal(ui.fieldInvalid, false);
}

// regexSettingsEqual の基本動作
function test_regex_settings_equal() {
    assert.equal(RegexPreview.regexSettingsEqual(validSettings, validSettings), true);
    assert.equal(
        RegexPreview.regexSettingsEqual(validSettings, { ...validSettings, pattern: 'x' }),
        false
    );
}

const tests = [
    test_stale_response_when_dom_changed,
    test_apply_response_when_dom_matches_request,
    test_ignore_outdated_response,
    test_validate_pending_after_stale_response,
    test_validate_uses_cache_after_apply,
    test_build_preview_ui_empty_pattern,
    test_regex_settings_equal,
];

for (const run of tests) {
    run();
}

console.log(`regex_preview_logic: ${tests.length} tests passed`);

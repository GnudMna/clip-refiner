/**
 * `SettingsSharedLogic` 純粋ロジックの Node テスト
 */
'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');

const SettingsSharedLogic = require(path.join(__dirname, '../src/ui/settings/shared_logic.js'));

const IMAGE_MODES = new Set(['excel-to-image']);

/** 件数入力をクランプできること */
function test_clamp_limit_value() {
    assert.deepEqual(SettingsSharedLogic.clampLimitValue('12', 1, 10), {
        value: 10,
        clamped: true,
    });
    assert.deepEqual(SettingsSharedLogic.clampLimitValue('abc', 3, 9), {
        value: 3,
        clamped: true,
    });
    assert.deepEqual(SettingsSharedLogic.clampLimitValue('7', 1, 10), {
        value: 7,
        clamped: false,
    });
}

/** HTML 特殊文字をエスケープできること */
function test_escape_html() {
    assert.equal(SettingsSharedLogic.escapeHtml('<b>&"'), '&lt;b&gt;&amp;&quot;');
}

/** 画像出力モードを末尾へ正規化できること */
function test_normalize_pipeline_moves_image_mode_to_tail() {
    const normalized = SettingsSharedLogic.normalizePipeline(
        ['trim', 'excel-to-image', 'url-decode', 'excel-to-image'],
        5,
        IMAGE_MODES
    );

    assert.deepEqual(normalized, ['trim', 'url-decode', 'excel-to-image']);
}

/** パイプライン長の上限を適用できること */
function test_normalize_pipeline_truncates_to_max_length() {
    const normalized = SettingsSharedLogic.normalizePipeline(['a', 'b', 'c', 'd'], 2, new Set());

    assert.deepEqual(normalized, ['a', 'b']);
}

/** 通知タブの DOM 状態から IPC パッチを組み立てられること */
function test_build_notification_settings_patch() {
    const patch = SettingsSharedLogic.buildNotificationSettingsPatch({
        'notify-enabled': false,
        'notify-process': true,
        'notify-ocr': false,
        'notify-result': true,
    });

    assert.deepEqual(patch, {
        enabled: false,
        notifyProcess: true,
        notifyOcr: false,
        notifyResult: true,
    });
}

/** マスター OFF 時でも個別トグルをパッチへ含められること */
function test_notification_patch_allows_sub_toggles_when_master_off() {
    const patch = SettingsSharedLogic.buildNotificationSettingsPatch({
        'notify-enabled': false,
        'notify-process': true,
        'notify-pause': true,
    });

    assert.equal(patch.enabled, false);
    assert.equal(patch.notifyProcess, true);
    assert.equal(patch.notifyPause, true);
}

/** スナップショット JSON の通知必須フィールドを検証できること */
function test_snapshot_has_notification_fields() {
    assert.equal(
        SettingsSharedLogic.snapshotHasNotificationFields({
            notificationSettings: {
                enabled: false,
                notifyProcess: true,
                notifyPause: true,
                notifyHistoryRestore: true,
                notifyOcr: true,
                notifyRegisteredClip: true,
                notifyFavorites: true,
                notifyContinuousRecord: true,
                notifyResult: false,
            },
        }),
        true
    );
    assert.equal(
        SettingsSharedLogic.snapshotHasNotificationFields({
            notificationSettings: { enabled: true },
        }),
        false
    );
}

const tests = [
    test_clamp_limit_value,
    test_escape_html,
    test_normalize_pipeline_moves_image_mode_to_tail,
    test_normalize_pipeline_truncates_to_max_length,
    test_build_notification_settings_patch,
    test_notification_patch_allows_sub_toggles_when_master_off,
    test_snapshot_has_notification_fields,
];

for (const test of tests) {
    test();
}

console.log(`settings_shared_logic: ${tests.length} passed`);

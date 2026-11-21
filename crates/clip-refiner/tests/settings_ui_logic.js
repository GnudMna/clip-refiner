/**
 * `SettingsPatch` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';

import SettingsPatch from '../src/ui/settings/settings_patch.js';

/** トップレベルキーをマージできること */
function test_merge_top_level_keys() {
    const merged = SettingsPatch.mergePatch(
        { historyEnabled: false },
        { continuousRecordEnabled: true }
    );

    assert.deepEqual(merged, {
        historyEnabled: false,
        continuousRecordEnabled: true,
    });
}

/** ネストしたオブジェクトを深くマージできること */
function test_merge_nested_objects() {
    const merged = SettingsPatch.mergePatch(
        { regex: { pattern: 'a', replacement: '' } },
        { regex: { replacement: 'b', caseInsensitive: true } }
    );

    assert.deepEqual(merged, {
        regex: {
            pattern: 'a',
            replacement: 'b',
            caseInsensitive: true,
        },
    });
}

/** 後勝ちでプリミティブ値を上書きできること */
function test_merge_overwrites_primitive_values() {
    const merged = SettingsPatch.mergePatch(
        { intervalMs: 1000, autoRefineEnabled: false },
        { intervalMs: 2000, autoRefineEnabled: true }
    );

    assert.deepEqual(merged, {
        intervalMs: 2000,
        autoRefineEnabled: true,
    });
}

/** 配列は置換マージされること */
function test_merge_replaces_arrays() {
    const merged = SettingsPatch.mergePatch(
        { pipeline: ['trim'] },
        { pipeline: ['url-decode', 'trim'] }
    );

    assert.deepEqual(merged, {
        pipeline: ['url-decode', 'trim'],
    });
}

/** `null` は未指定としてスキップし既存値を維持すること */
function test_merge_skips_null_values() {
    const merged = SettingsPatch.mergePatch(
        {
            historyEnabled: true,
            notificationSettings: {
                enabled: false,
                notifyProcess: true,
            },
        },
        {
            historyEnabled: null,
            notificationSettings: {
                enabled: true,
                notifyAutoRefine: null,
                notifyResult: true,
            },
            intervalMs: 2000,
        }
    );

    assert.deepEqual(merged, {
        historyEnabled: true,
        intervalMs: 2000,
        notificationSettings: {
            enabled: true,
            notifyProcess: true,
            notifyResult: true,
        },
    });
}

const tests = [
    test_merge_top_level_keys,
    test_merge_nested_objects,
    test_merge_overwrites_primitive_values,
    test_merge_replaces_arrays,
    test_merge_skips_null_values,
];

for (const test of tests) {
    test();
}

console.log(`settings_ui_logic: ${tests.length} passed`);

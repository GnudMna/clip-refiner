/**
 * `SettingsPatch` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';
import { test } from 'node:test';

import SettingsPatch from '../../src/ui/settings/settings_patch.ts';

/** トップレベルキーをマージできること */
test('merge_top_level_keys', () => {
    const merged = SettingsPatch.mergePatch(
        { historyEnabled: false },
        { continuousRecordEnabled: true }
    );

    assert.deepEqual(merged, {
        historyEnabled: false,
        continuousRecordEnabled: true,
    });
});

/** ネストしたオブジェクトを深くマージできること */
test('merge_nested_objects', () => {
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
});

/** 後勝ちでプリミティブ値を上書きできること */
test('merge_overwrites_primitive_values', () => {
    const merged = SettingsPatch.mergePatch(
        { intervalMs: 1000, autoRefineEnabled: false },
        { intervalMs: 2000, autoRefineEnabled: true }
    );

    assert.deepEqual(merged, {
        intervalMs: 2000,
        autoRefineEnabled: true,
    });
});

/** 配列は置換マージされること */
test('merge_replaces_arrays', () => {
    const merged = SettingsPatch.mergePatch(
        { pipeline: ['trim'] },
        { pipeline: ['url-decode', 'trim'] }
    );

    assert.deepEqual(merged, {
        pipeline: ['url-decode', 'trim'],
    });
});

/** `null` は未指定としてスキップし既存値を維持すること */
test('merge_skips_null_values', () => {
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
});

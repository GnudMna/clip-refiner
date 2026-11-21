/**
 * `SettingsSharedLogic` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';

import SettingsSharedLogic from '../src/ui/settings/shared_logic.js';

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

/** ホットキー文字列のクライアント検証ができること */
function test_validate_hotkey_binding() {
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift+S').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Ctrl+F12').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift+Backspace').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('').valid, false);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('', { allowEmpty: true }).valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift').valid, false);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Enter').valid, false);
}

/** ホットキー DOM 状態から IPC パッチを組み立てられること */
function test_build_hotkeys_patch() {
    const patch = SettingsSharedLogic.buildHotkeysPatch(
        {
            'hotkey-quick-selector': 'Ctrl+Shift+S',
            'hotkey-pause': 'Alt+Shift+P',
        },
        ['', 'Alt+Shift+2']
    );

    assert.equal(patch.quickSelector, 'Ctrl+Shift+S');
    assert.equal(patch.pause, 'Alt+Shift+P');
    assert.deepEqual(patch.favoriteModeSlots, ['', 'Alt+Shift+2']);
}

/** スナップショット JSON のホットキー必須フィールドを検証できること */
function test_snapshot_has_hotkey_fields() {
    assert.equal(
        SettingsSharedLogic.snapshotHasHotkeyFields({
            hotkeys: {
                quickSelector: 'Alt+Shift+S',
                clipSelector: 'Alt+Shift+T',
                ocr: 'Alt+Shift+G',
                notification: 'Alt+Shift+N',
                pause: 'Alt+Shift+P',
                continuousRecord: 'Alt+Shift+C',
                continuousRecordUndo: 'Alt+Shift+Backspace',
                continuousRecordPaste: 'Alt+Shift+X',
                undo: 'Alt+Shift+Z',
                quit: 'Alt+Shift+Q',
                favoriteModeSlots: [],
            },
        }),
        true
    );
    assert.equal(
        SettingsSharedLogic.snapshotHasHotkeyFields({
            hotkeys: { quickSelector: 'Alt+Shift+S' },
        }),
        false
    );
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

/** force / externalSync 時はフル再描画すること */
function test_should_full_render_on_force_or_external_sync() {
    assert.equal(
        SettingsSharedLogic.shouldFullRenderOnLoad({ force: true }, true, 'regex', false),
        true
    );
    assert.equal(
        SettingsSharedLogic.shouldFullRenderOnLoad({ externalSync: true }, true, 'regex', false),
        true
    );
}

/** 編集中でもお気に入りはフル再描画し、ショートカットはフォーカス維持のため省略すること */
function test_should_full_render_while_editing_favorites_or_shortcuts() {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'favorites', false), true);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'shortcuts', false), false);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'regex', false), false);
}

/** 登録クリップ処理中は clips タブでフル再描画すること */
function test_should_full_render_when_clip_register_pending_on_clips_tab() {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'clips', true), true);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'regex', true), false);
}

/** 未編集時はフル再描画すること */
function test_should_full_render_when_not_editing() {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, false, 'regex', false), true);
}

/** ショートカット編集中は入力欄同期、件数変化時は再描画すること */
function test_resolve_shortcuts_diff_update() {
    assert.equal(
        SettingsSharedLogic.resolveShortcutsDiffUpdate({
            wasEditingShortcuts: true,
            previousFavoriteCount: 2,
            nextFavoriteCount: 2,
        }),
        'syncInputs'
    );
    assert.equal(
        SettingsSharedLogic.resolveShortcutsDiffUpdate({
            wasEditingShortcuts: true,
            previousFavoriteCount: 2,
            nextFavoriteCount: 3,
        }),
        'refreshPanel'
    );
    assert.equal(
        SettingsSharedLogic.resolveShortcutsDiffUpdate({
            wasEditingShortcuts: false,
            previousFavoriteCount: 2,
            nextFavoriteCount: 2,
        }),
        'refreshPanel'
    );
}

/** Escape はダイアログ・編集中以外で閉じること */
function test_should_close_settings_on_escape() {
    assert.equal(
        SettingsSharedLogic.shouldCloseSettingsOnEscape({
            dialogVisible: false,
            isEditing: false,
        }),
        true
    );
    assert.equal(
        SettingsSharedLogic.shouldCloseSettingsOnEscape({
            dialogVisible: true,
            isEditing: false,
        }),
        false
    );
    assert.equal(
        SettingsSharedLogic.shouldCloseSettingsOnEscape({
            dialogVisible: false,
            isEditing: true,
        }),
        false
    );
}

/** 閉じる操作は保留パッチを先に apply してから close すること */
function test_collect_close_window_ipc_messages() {
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages(null), [{ type: 'close' }]);
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages({}), [{ type: 'close' }]);
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages({ intervalMs: 750 }), [
        { type: 'apply', patch: { intervalMs: 750 } },
        { type: 'close' },
    ]);
}

/** 設定シェル HTML の必須 DOM ID を検証できること */
function test_settings_shell_has_required_ids() {
    const html =
        '<div id="app"><div id="sidebar"></div><main id="content"></main>' +
        '<button id="open-config-btn"></button><button id="close-btn"></button>' +
        '<div id="apply-feedback"></div><div id="confirm-dialog"></div></div>';
    assert.equal(SettingsSharedLogic.settingsShellHasRequiredIds(html), true);
    assert.equal(SettingsSharedLogic.settingsShellHasRequiredIds('<div id="app"></div>'), false);
}

/** 組み立て JS に全タブ定義が含まれることを検証できること */
function test_settings_js_defines_all_tabs() {
    const singleQuoted = SettingsSharedLogic.SETTINGS_TAB_IDS.map(
        (tabId) => "id: '" + tabId + "'"
    ).join('\n');
    const doubleQuoted = SettingsSharedLogic.SETTINGS_TAB_IDS.map(
        (tabId) => 'id: "' + tabId + '"'
    ).join('\n');
    assert.equal(SettingsSharedLogic.settingsJsDefinesAllTabs(singleQuoted), true);
    assert.equal(SettingsSharedLogic.settingsJsDefinesAllTabs(doubleQuoted), true);
    assert.equal(SettingsSharedLogic.settingsJsDefinesAllTabs("id: 'general'"), false);
    assert.equal(SettingsSharedLogic.settingsJsDefinesAllTabs('id: "general"'), false);
}

const tests = [
    test_clamp_limit_value,
    test_escape_html,
    test_normalize_pipeline_moves_image_mode_to_tail,
    test_normalize_pipeline_truncates_to_max_length,
    test_build_notification_settings_patch,
    test_validate_hotkey_binding,
    test_build_hotkeys_patch,
    test_notification_patch_allows_sub_toggles_when_master_off,
    test_snapshot_has_notification_fields,
    test_snapshot_has_hotkey_fields,
    test_should_full_render_on_force_or_external_sync,
    test_should_full_render_while_editing_favorites_or_shortcuts,
    test_should_full_render_when_clip_register_pending_on_clips_tab,
    test_should_full_render_when_not_editing,
    test_resolve_shortcuts_diff_update,
    test_should_close_settings_on_escape,
    test_collect_close_window_ipc_messages,
    test_settings_shell_has_required_ids,
    test_settings_js_defines_all_tabs,
];

for (const test of tests) {
    test();
}

console.log(`settings_shared_logic: ${tests.length} passed`);

/**
 * `SettingsSharedLogic` 純粋ロジックの Node テスト
 */
import assert from 'node:assert/strict';
import { test } from 'node:test';

import SettingsSharedLogic from '../../src/ui/settings/shared_logic.ts';

const IMAGE_MODES = new Set(['excel-to-image']);

/** 件数入力をクランプできること */
test('clamp_limit_value', () => {
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
});

/** HTML 特殊文字をエスケープできること */
test('escape_html', () => {
    assert.equal(SettingsSharedLogic.escapeHtml('<b>&"'), '&lt;b&gt;&amp;&quot;');
});

/** 画像出力モードを末尾へ正規化できること */
test('normalize_pipeline_moves_image_mode_to_tail', () => {
    const normalized = SettingsSharedLogic.normalizePipeline(
        ['trim', 'excel-to-image', 'url-decode', 'excel-to-image'],
        5,
        IMAGE_MODES
    );

    assert.deepEqual(normalized, ['trim', 'url-decode', 'excel-to-image']);
});

/** パイプライン長の上限を適用できること */
test('normalize_pipeline_truncates_to_max_length', () => {
    const normalized = SettingsSharedLogic.normalizePipeline(['a', 'b', 'c', 'd'], 2, new Set());

    assert.deepEqual(normalized, ['a', 'b']);
});

/** 上限を超えても末尾の画像出力モードを残し、テキスト加工の超過分を切り捨てること */
test('normalize_pipeline_keeps_image_mode_when_truncating', () => {
    const normalized = SettingsSharedLogic.normalizePipeline(
        ['excel-to-image', 'a', 'b', 'c'],
        3,
        IMAGE_MODES
    );

    assert.deepEqual(normalized, ['a', 'b', 'excel-to-image']);
});

/** 通知タブの DOM 状態から IPC パッチを組み立てられること */
test('build_notification_settings_patch', () => {
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
});

/** ホットキー文字列のクライアント検証ができること */
test('validate_hotkey_binding', () => {
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift+S').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Ctrl+F12').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift+Backspace').valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('').valid, false);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('', { allowEmpty: true }).valid, true);
    assert.equal(SettingsSharedLogic.validateHotkeyBinding('Alt+Shift').valid, false);
    for (const binding of [
        'Alt+Enter',
        'Ctrl+F24',
        'Alt+Shift+Up',
        'Ctrl+Numpad5',
        'Ctrl+NumpadAdd',
        'Alt+Shift+-',
        'Alt+Shift+Esc',
    ]) {
        assert.equal(SettingsSharedLogic.validateHotkeyBinding(binding).valid, true, binding);
    }
    for (const binding of ['Alt+F25', 'Alt+;', 'Alt+CapsLock', 'Alt+Numpad10']) {
        assert.equal(SettingsSharedLogic.validateHotkeyBinding(binding).valid, false, binding);
    }
});

/** ホットキー DOM 状態から IPC パッチを組み立てられること */
test('build_hotkeys_patch', () => {
    const patch = SettingsSharedLogic.buildHotkeysPatch(
        {
            'hotkey-quick-selector': 'Ctrl+Shift+S',
            'hotkey-auto-refine': 'Alt+Shift+P',
        },
        ['', 'Alt+Shift+2']
    );

    assert.equal(patch.quickSelector, 'Ctrl+Shift+S');
    assert.equal(patch.autoRefine, 'Alt+Shift+P');
    assert.deepEqual(patch.favoriteModeSlots, ['', 'Alt+Shift+2']);
});

/** スナップショット JSON のホットキー必須フィールドを検証できること */
test('snapshot_has_hotkey_fields', () => {
    assert.equal(
        SettingsSharedLogic.snapshotHasHotkeyFields({
            hotkeys: {
                quickSelector: 'Alt+Shift+S',
                clipSelector: 'Alt+Shift+T',
                ocr: 'Alt+Shift+G',
                notification: 'Alt+Shift+N',
                autoRefine: 'Alt+Shift+P',
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
});

/** マスター OFF 時でも個別トグルをパッチへ含められること */
test('notification_patch_allows_sub_toggles_when_master_off', () => {
    const patch = SettingsSharedLogic.buildNotificationSettingsPatch({
        'notify-enabled': false,
        'notify-process': true,
        'notify-auto-refine': true,
    });

    assert.equal(patch.enabled, false);
    assert.equal(patch.notifyProcess, true);
    assert.equal(patch.notifyAutoRefine, true);
});

/** スナップショット JSON の通知必須フィールドを検証できること */
test('snapshot_has_notification_fields', () => {
    assert.equal(
        SettingsSharedLogic.snapshotHasNotificationFields({
            notificationSettings: {
                enabled: false,
                notifyProcess: true,
                notifyAutoRefine: true,
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
});

/** force / externalSync 時はフル再描画すること */
test('should_full_render_on_force_or_external_sync', () => {
    assert.equal(
        SettingsSharedLogic.shouldFullRenderOnLoad({ force: true }, true, 'regex', false),
        true
    );
    assert.equal(
        SettingsSharedLogic.shouldFullRenderOnLoad({ externalSync: true }, true, 'regex', false),
        true
    );
});

/** 編集中でも変換タブはフル再描画し、ショートカットはフォーカス維持のため省略すること */
test('should_full_render_while_editing_favorites_or_shortcuts', () => {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'convert', false), true);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'shortcuts', false), false);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'regex', false), false);
});

/** 登録クリップ処理中はクリップボードタブでフル再描画すること */
test('should_full_render_when_clip_register_pending_on_clips_tab', () => {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'clipboard', true), true);
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, true, 'regex', true), false);
});

/** 未編集時はフル再描画すること */
test('should_full_render_when_not_editing', () => {
    assert.equal(SettingsSharedLogic.shouldFullRenderOnLoad({}, false, 'regex', false), true);
});

/** ショートカット編集中は入力欄同期、件数変化時は再描画すること */
test('resolve_shortcuts_diff_update', () => {
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
});

/** Escape はダイアログ・編集中以外で閉じること */
test('should_close_settings_on_escape', () => {
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
});

/** 閉じる操作は保留パッチを先に apply してから close すること */
test('collect_close_window_ipc_messages', () => {
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages(null), [{ type: 'close' }]);
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages({}), [{ type: 'close' }]);
    assert.deepEqual(SettingsSharedLogic.collectCloseWindowIpcMessages({ intervalMs: 750 }), [
        { type: 'apply', patch: { intervalMs: 750 } },
        { type: 'close' },
    ]);
});

/** 設定シェル HTML の必須 DOM ID を検証できること */
test('settings_shell_has_required_ids', () => {
    const html =
        '<div id="app"><div id="sidebar"></div><main id="content"></main>' +
        '<button id="open-config-btn"></button><button id="close-btn"></button>' +
        '<div id="apply-feedback"></div><div id="confirm-dialog"></div></div>';
    assert.equal(SettingsSharedLogic.settingsShellHasRequiredIds(html), true);
    assert.equal(SettingsSharedLogic.settingsShellHasRequiredIds('<div id="app"></div>'), false);
});

/** 組み立て JS に全タブ定義が含まれることを検証できること */
test('settings_js_defines_all_tabs', () => {
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
});

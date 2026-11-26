// ======================================================================
// 設定画面 イベントバインド
// ======================================================================

import { eventInput, inputById, queryInputs, selectById } from '../shared/dom.ts';
import { showConfirmDialog } from './dialogs/confirm_dialog.ts';
import { ctx, loadedState } from './ctx.ts';
import {
    postIpcEnvelope,
    openConfigFile,
    openDocument,
    scheduleApply,
    showApplyFeedback,
} from './ipc_apply.ts';
import {
    applyPipelineChange,
    syncContinuousRecordUi,
    syncMonitorIntervalSection,
} from './monitor_ui.ts';
import {
    readRegexForApply,
    scheduleRegexFeedback,
    updateRegexFeedback,
} from './regex/regex_feedback.ts';
import { validateRegexSettings } from './regex/regex_validation.ts';
import { emptyStep, moveListItem, newPresetId, selectedRegexPresetId } from './regex/regex_ui.ts';
import {
    historyLimitBounds,
    intervalMsBounds,
    maxPipelineLength,
    maxRegexPresets,
    maxRegexStepsPerPreset,
    recordLimitBounds,
    selectorWebviewIdleMinutesBounds,
} from './limits.ts';
import {
    commitLimitInput,
    setClipRegisterPending,
    syncSelectorWebviewIdleSection,
} from './shared.ts';

import SettingsPanelsLogic from './panels_logic.ts';
import SettingsSharedLogic from './shared_logic.ts';

import type { MonitorMode, SelectorWebviewRetention } from '../ipc.ts';

/**
 * 加工パイプライン行の上下移動・削除ボタンを処理する
 */
function handlePipelineAction(button: HTMLButtonElement): void {
    const index = Number(button.dataset.index);
    let action: 'up' | 'down' | 'remove' = 'remove';
    if (button.classList.contains('pipeline-up')) {
        action = 'up';
    } else if (button.classList.contains('pipeline-down')) {
        action = 'down';
    }

    const next = SettingsPanelsLogic.applyPipelineAction(
        loadedState().pipeline || [],
        index,
        action
    );
    if (!next) {
        return;
    }
    applyPipelineChange(next);
}

/**
 * コンテンツ領域のクリックを委譲処理する
 */
function handleContentClick(event: MouseEvent): void {
    const button = (event.target as Element).closest('button');
    if (!button) {
        return;
    }

    const resolved = SettingsPanelsLogic.resolveContentClickAction(button);
    if (!resolved) {
        return;
    }

    if (resolved.type === 'pipeline') {
        handlePipelineAction(button);
        return;
    }

    if (resolved.type === 'regex-action') {
        handleRegexAction(button);
        return;
    }

    if (resolved.type === 'favorite-toggle') {
        postIpcEnvelope({ type: 'favorite-toggle', mode: resolved.mode });
        return;
    }

    if (resolved.type === 'favorite-move') {
        postIpcEnvelope({
            type: 'favorite-move',
            mode: resolved.mode,
            direction: resolved.direction,
        });
        return;
    }

    if (resolved.type === 'clip-move') {
        postIpcEnvelope({
            type: 'clip-move',
            index: resolved.index,
            token: resolved.token,
            direction: resolved.direction,
        });
        return;
    }

    if (resolved.type === 'clip-template') {
        postIpcEnvelope({
            type: 'clip-template',
            index: resolved.index,
            token: resolved.token,
            enabled: resolved.enabled,
        });
        return;
    }

    if (resolved.type === 'clip-counter-reset') {
        postIpcEnvelope({
            type: 'clip-counter-reset',
            index: resolved.index,
            token: resolved.token,
        });
        return;
    }

    if (resolved.type === 'clip-edit') {
        postIpcEnvelope({
            type: 'clip-edit-open',
            index: resolved.index,
            token: resolved.token,
        });
        return;
    }

    if (resolved.type === 'clip-copy') {
        postIpcEnvelope({
            type: 'clip-copy',
            index: resolved.index,
            token: resolved.token,
        });
        return;
    }

    if (resolved.type === 'clip-delete') {
        showConfirmDialog({
            message: 'この登録クリップを削除しますか？',
            confirmLabel: '削除する',
            onConfirm: () => {
                postIpcEnvelope({
                    type: 'clip-delete',
                    index: resolved.index,
                    token: resolved.token,
                });
            },
        });
    }
}

/** コンテンツ領域へのクリック委譲を一度だけバインドする */
export function bindContentDelegation(): void {
    if (ctx.contentDelegationBound) {
        return;
    }
    document.getElementById('content')?.addEventListener('click', handleContentClick);
    ctx.contentDelegationBound = true;
}

/** 監視・自動変換のフォームイベントをバインドする (変換タブと監視タブで共有) */
function bindMonitorEvents(): void {
    document.getElementById('auto-refine-enabled')?.addEventListener('change', (event) => {
        loadedState().autoRefineEnabled = eventInput(event).checked;
        scheduleApply({ autoRefineEnabled: eventInput(event).checked });
    });

    queryInputs('input[name="monitorMode"]').forEach((input) => {
        input.addEventListener('change', () => {
            loadedState().monitorMode = input.value as MonitorMode;
            scheduleApply({ monitorMode: loadedState().monitorMode });
            syncMonitorIntervalSection();
        });
    });

    const intervalInput = inputById('interval-ms');
    if (intervalInput) {
        const bounds = intervalMsBounds();
        /** ポーリング間隔入力をコミットする */
        const commitIntervalMs = () =>
            commitLimitInput(intervalInput, 'intervalMs', 'intervalMs', bounds);
        intervalInput.addEventListener('change', commitIntervalMs);
        intervalInput.addEventListener('blur', commitIntervalMs);
    }

    const defaultMode = selectById('default-mode');
    if (defaultMode) {
        defaultMode.addEventListener('change', () => {
            loadedState().mode = defaultMode.value;
            scheduleApply({ mode: defaultMode.value });
        });
    }

    document.getElementById('pipeline-add-btn')?.addEventListener('click', () => {
        const select = selectById('pipeline-add-mode');
        if (!select) {
            return;
        }
        const modeId = select.value;
        const next = SettingsPanelsLogic.appendPipelineMode(
            loadedState().pipeline || [],
            modeId,
            maxPipelineLength()
        );
        if (!next) {
            return;
        }
        applyPipelineChange(next);
    });

    document.getElementById('pipeline-clear-btn')?.addEventListener('click', () => {
        applyPipelineChange([]);
    });
}

/** 通知タブのフォームイベントをバインドする */
function bindNotificationEvents(): void {
    /** DOM の通知設定を読み取りパッチをスケジュールする */
    function applyNotifications(): void {
        const fieldStates: Record<string, boolean> = {};
        for (const [elementId] of SettingsSharedLogic.NOTIFICATION_FIELD_MAP) {
            const element = inputById(elementId);
            if (element) {
                fieldStates[elementId] = element.checked;
            }
        }
        scheduleApply({
            notificationSettings: SettingsSharedLogic.buildNotificationSettingsPatch(fieldStates),
        });
    }

    for (const [elementId, key] of SettingsSharedLogic.NOTIFICATION_FIELD_MAP) {
        document.getElementById(elementId)?.addEventListener('change', (event) => {
            loadedState().notificationSettings[key] = eventInput(event).checked;
            applyNotifications();
        });
    }
}

/** クリップボードタブの履歴・連続記録フォームイベントをバインドする */
function bindHistoryEvents(): void {
    document.getElementById('history-enabled')?.addEventListener('change', (event) => {
        loadedState().historyEnabled = eventInput(event).checked;
        scheduleApply({ historyEnabled: eventInput(event).checked });
    });
    document.getElementById('keep-exclusion')?.addEventListener('change', (event) => {
        loadedState().keepClipboardExclusion = eventInput(event).checked;
        scheduleApply({ keepClipboardExclusion: eventInput(event).checked });
    });
    document.getElementById('record-enabled')?.addEventListener('change', (event) => {
        loadedState().continuousRecordEnabled = eventInput(event).checked;
        scheduleApply({ continuousRecordEnabled: eventInput(event).checked });
        syncContinuousRecordUi();
    });

    const historyLimit = inputById('history-limit');
    if (historyLimit) {
        const bounds = historyLimitBounds();
        /** 履歴保持件数入力をコミットする */
        const commitHistoryLimit = () =>
            commitLimitInput(historyLimit, 'historyLimit', 'historyLimit', bounds);
        historyLimit.addEventListener('change', commitHistoryLimit);
        historyLimit.addEventListener('blur', commitHistoryLimit);
    }

    const recordLimit = inputById('record-limit');
    if (recordLimit) {
        const bounds = recordLimitBounds();
        /** 連続記録保持件数入力をコミットする */
        const commitRecordLimit = () =>
            commitLimitInput(recordLimit, 'continuousRecordLimit', 'continuousRecordLimit', bounds);
        recordLimit.addEventListener('change', commitRecordLimit);
        recordLimit.addEventListener('blur', commitRecordLimit);
    }

    document.getElementById('clear-history-btn')?.addEventListener('click', () => {
        showConfirmDialog({
            message: '履歴をすべて削除しますか？\nこの操作は元に戻せません。',
            confirmLabel: '削除する',
            onConfirm: () => postIpcEnvelope({ type: 'clear-history' }),
        });
    });
    document.getElementById('clear-record-btn')?.addEventListener('click', () => {
        showConfirmDialog({
            message: '連続記録モードの記録をすべて削除しますか？\nこの操作は元に戻せません。',
            confirmLabel: '削除する',
            onConfirm: () => postIpcEnvelope({ type: 'clear-record' }),
        });
    });
}

/** 正規表現タブのフォームイベントをバインドする */
function bindRegexEvents(): void {
    /** DOM の正規表現設定を検証し、有効ならパッチをスケジュールする */
    function applyRegex(): void {
        const settings = readRegexForApply();
        const validation = validateRegexSettings(settings);
        updateRegexFeedback();
        if (!validation.valid) {
            return;
        }
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
    }

    document.getElementById('regex-preset-name')?.addEventListener('input', () => {
        scheduleRegexFeedback();
    });
    document.getElementById('regex-preset-name')?.addEventListener('blur', applyRegex);

    document.querySelectorAll('.regex-step-pattern, .regex-step-replacement').forEach((el) => {
        el.addEventListener('input', scheduleRegexFeedback);
        el.addEventListener('blur', applyRegex);
    });
    document.querySelectorAll('[id^="regex-step-ci-"], [id^="regex-step-ml-"]').forEach((el) => {
        el.addEventListener('change', () => {
            scheduleRegexFeedback();
            applyRegex();
        });
    });
    document
        .getElementById('regex-preview-sample')
        ?.addEventListener('input', scheduleRegexFeedback);

    queryInputs('.regex-active-radio').forEach((el) => {
        el.addEventListener('change', () => {
            const presetId = el.dataset.id;
            if (!presetId) {
                return;
            }
            const settings = readRegexForApply();
            settings.activePresetId = presetId;
            loadedState().regex = settings;
            scheduleApply({ regex: settings });
            updateRegexFeedback();
        });
    });

    document.getElementById('regex-preset-add-btn')?.addEventListener('click', () => {
        const settings = readRegexForApply();
        if (settings.presets.length >= maxRegexPresets()) {
            return;
        }
        const id = newPresetId();
        settings.presets.push({
            id,
            name: 'プリセット ' + (settings.presets.length + 1),
            steps: [emptyStep()],
        });
        ctx.selectedRegexPresetId = id;
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
    });

    document.getElementById('regex-step-add-btn')?.addEventListener('click', () => {
        const settings = readRegexForApply();
        const selectedId = selectedRegexPresetId();
        const preset = settings.presets.find((item) => item.id === selectedId);
        if (!preset || preset.steps.length >= maxRegexStepsPerPreset()) {
            return;
        }
        preset.steps.push(emptyStep());
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
    });

    updateRegexFeedback();
}

/**
 * 正規表現プリセット / ステップの一覧操作を処理する
 */
function handleRegexAction(button: HTMLButtonElement): void {
    if (button.classList.contains('regex-preset-select')) {
        ctx.selectedRegexPresetId = button.dataset.id ?? null;
        ctx.refreshActivePanel();
        return;
    }

    const settings = readRegexForApply();
    const selectedId = selectedRegexPresetId();

    if (
        button.classList.contains('regex-preset-up') ||
        button.classList.contains('regex-preset-down')
    ) {
        const index = Number(button.dataset.index);
        const direction = button.classList.contains('regex-preset-up') ? 'up' : 'down';
        settings.presets = moveListItem(settings.presets, index, direction);
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
        return;
    }

    if (button.classList.contains('regex-preset-remove')) {
        const index = Number(button.dataset.index);
        if (settings.presets.length <= 1 || Number.isNaN(index)) {
            return;
        }
        const removed = settings.presets.splice(index, 1)[0];
        if (!removed) {
            return;
        }
        if (settings.activePresetId === removed.id) {
            settings.activePresetId = settings.presets[0]?.id ?? settings.activePresetId;
        }
        if (ctx.selectedRegexPresetId === removed.id) {
            ctx.selectedRegexPresetId = settings.activePresetId;
        }
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
        return;
    }

    const preset = settings.presets.find((item) => item.id === selectedId);
    if (!preset) {
        return;
    }

    if (
        button.classList.contains('regex-step-up') ||
        button.classList.contains('regex-step-down')
    ) {
        const index = Number(button.dataset.index);
        const direction = button.classList.contains('regex-step-up') ? 'up' : 'down';
        preset.steps = moveListItem(preset.steps, index, direction);
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
        return;
    }

    if (button.classList.contains('regex-step-remove')) {
        const index = Number(button.dataset.index);
        if (preset.steps.length <= 1 || Number.isNaN(index)) {
            return;
        }
        preset.steps.splice(index, 1);
        loadedState().regex = settings;
        scheduleApply({ regex: settings });
        ctx.refreshActivePanel();
    }
}

/** ショートカットタブのイベントをバインドする */
function bindShortcutsEvents(): void {
    document.getElementById('shortcuts-open-config-btn')?.addEventListener('click', openConfigFile);

    /** DOM からホットキーパッチを組み立てて適用する */
    function applyHotkeysFromDom(): void {
        const fixedFieldStates: Record<string, string> = {};
        let valid = true;
        for (const [elementId] of SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP) {
            const element = inputById(elementId);
            if (!element) {
                continue;
            }
            const value = element.value.trim();
            fixedFieldStates[elementId] = value;
            const validation = SettingsSharedLogic.validateHotkeyBinding(value);
            element.closest('.field')?.classList.toggle('field-invalid', !validation.valid);
            element.classList.toggle('is-invalid', !validation.valid);
            if (!validation.valid) {
                valid = false;
                showApplyFeedback({
                    ok: false,
                    field: 'hotkeys',
                    message: validation.message,
                });
            }
        }

        const favoriteModes = loadedState().favoriteModes || [];
        const favoriteSlotBindings = [];
        for (let index = 0; index < favoriteModes.length; index += 1) {
            const element = inputById('hotkey-favorite-slot-' + index);
            const value = element ? element.value.trim() : '';
            favoriteSlotBindings.push(value);
            if (element) {
                const validation = SettingsSharedLogic.validateHotkeyBinding(value, {
                    allowEmpty: true,
                });
                element.classList.toggle('is-invalid', !validation.valid);
                if (!validation.valid) {
                    valid = false;
                    showApplyFeedback({
                        ok: false,
                        field: 'hotkeys',
                        message: validation.message,
                    });
                }
            }
        }

        if (!valid) {
            return;
        }

        const hotkeys = SettingsSharedLogic.buildHotkeysPatch(
            fixedFieldStates,
            favoriteSlotBindings
        );
        loadedState().hotkeys = Object.assign({}, loadedState().hotkeys, hotkeys);
        scheduleApply({ hotkeys });
    }

    queryInputs('.shortcut-input').forEach((input) => {
        input.addEventListener('blur', applyHotkeysFromDom);
        input.addEventListener('keydown', (event) => {
            if (event.key === 'Enter') {
                event.preventDefault();
                input.blur();
            }
        });
    });
}

/** お気に入りタブのイベントをバインドする */
function bindFavoriteEvents(): void {
    document.getElementById('favorite-add-btn')?.addEventListener('click', () => {
        const modeId = selectById('favorite-add-mode')?.value;
        if (!modeId) {
            return;
        }
        postIpcEnvelope({ type: 'favorite-toggle', mode: modeId });
    });
}

/** 一般タブのイベントをバインドする */
function bindGeneralEvents(): void {
    document.querySelector('.advanced-section')?.addEventListener('toggle', (event) => {
        ctx.advancedSectionOpen = (event.target as HTMLDetailsElement).open;
    });

    document.getElementById('launch-at-login')?.addEventListener('change', (event) => {
        loadedState().launchAtLogin = eventInput(event).checked;
        scheduleApply({ launchAtLogin: eventInput(event).checked });
    });

    queryInputs('input[name="selectorWebviewRetention"]').forEach((input) => {
        input.addEventListener('change', () => {
            loadedState().selectorWebviewRetention = input.value as SelectorWebviewRetention;
            scheduleApply({ selectorWebviewRetention: loadedState().selectorWebviewRetention });
            syncSelectorWebviewIdleSection();
        });
    });

    const idleInput = inputById('selector-webview-idle-minutes');
    if (idleInput) {
        const bounds = selectorWebviewIdleMinutesBounds();
        /** アイドル分数入力をコミットする */
        const commitIdleMinutes = () =>
            commitLimitInput(
                idleInput,
                'selectorWebviewIdleMinutes',
                'selectorWebviewIdleMinutes',
                bounds
            );
        idleInput.addEventListener('change', commitIdleMinutes);
        idleInput.addEventListener('blur', commitIdleMinutes);
    }

    document.getElementById('check-update-btn')?.addEventListener('click', () => {
        postIpcEnvelope({ type: 'check-update' });
    });
}

/** クリップボードタブの登録クリップイベントをバインドする */
function bindClipEvents(): void {
    document.getElementById('clip-register-btn')?.addEventListener('click', () => {
        if (ctx.clipRegisterPending) {
            return;
        }
        setClipRegisterPending(true);
        postIpcEnvelope({ type: 'clip-register' });
    });
}

/** 情報タブのイベントをバインドする */
function bindAboutEvents(): void {
    document.getElementById('open-user-docs-btn')?.addEventListener('click', () => {
        openDocument('user-docs');
    });
    document.getElementById('open-license-btn')?.addEventListener('click', () => {
        openDocument('license');
    });
    document.getElementById('open-third-party-notices-btn')?.addEventListener('click', () => {
        openDocument('third-party-notices');
    });
}

/** アクティブタブに応じたパネルイベントをバインドする */
export function bindActivePanelEvents(): void {
    switch (ctx.activeTab) {
        case 'monitor':
            bindMonitorEvents();
            break;
        case 'convert':
            bindMonitorEvents();
            bindFavoriteEvents();
            break;
        case 'regex':
            bindRegexEvents();
            break;
        case 'clipboard':
            bindHistoryEvents();
            bindClipEvents();
            break;
        case 'notifications':
            bindNotificationEvents();
            break;
        case 'shortcuts':
            bindShortcutsEvents();
            break;
        case 'general':
            bindGeneralEvents();
            break;
        case 'about':
            bindAboutEvents();
            break;
        default:
            break;
    }
}

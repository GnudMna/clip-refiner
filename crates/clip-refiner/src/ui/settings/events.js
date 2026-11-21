// ======================================================================
// 設定画面 イベントバインド
// ======================================================================

/**
 * 加工パイプライン行の上下移動・削除ボタンを処理する
 *
 * @param {HTMLButtonElement} button
 */
function handlePipelineAction(button) {
    const index = Number(button.dataset.index);
    let action = 'remove';
    if (button.classList.contains('pipeline-up')) {
        action = 'up';
    } else if (button.classList.contains('pipeline-down')) {
        action = 'down';
    }

    const next = SettingsPanelsLogic.applyPipelineAction(state.pipeline || [], index, action);
    if (!next) {
        return;
    }
    applyPipelineChange(next);
}

/**
 * コンテンツ領域のクリックを委譲処理する
 *
 * @param {MouseEvent} event
 */
function handleContentClick(event) {
    const button = event.target.closest('button');
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

    if (resolved.type === 'clip-copy') {
        postIpcEnvelope({ type: 'clip-copy', index: resolved.index });
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
                });
            },
        });
    }
}

/** コンテンツ領域へのクリック委譲を一度だけバインドする */
function bindContentDelegation() {
    if (contentDelegationBound) {
        return;
    }
    document.getElementById('content')?.addEventListener('click', handleContentClick);
    contentDelegationBound = true;
}

/** 監視タブのフォームイベントをバインドする */
function bindMonitorEvents() {
    document.getElementById('monitor-paused')?.addEventListener('change', (event) => {
        state.isPaused = event.target.checked;
        scheduleApply({ isPaused: event.target.checked });
        syncMonitorPauseBanner();
    });

    document.querySelectorAll('input[name="monitorMode"]').forEach((input) => {
        input.addEventListener('change', () => {
            state.monitorMode = input.value;
            scheduleApply({ monitorMode: state.monitorMode });
            syncMonitorIntervalSection();
        });
    });

    const intervalInput = document.getElementById('interval-ms');
    if (intervalInput) {
        const bounds = intervalMsBounds();
        /** ポーリング間隔入力をコミットする */
        const commitIntervalMs = () =>
            commitLimitInput(intervalInput, 'intervalMs', 'intervalMs', bounds);
        intervalInput.addEventListener('change', commitIntervalMs);
        intervalInput.addEventListener('blur', commitIntervalMs);
    }

    const defaultMode = document.getElementById('default-mode');
    if (defaultMode) {
        defaultMode.addEventListener('change', () => {
            scheduleApply({ mode: defaultMode.value });
        });
    }

    document.getElementById('pipeline-add-btn')?.addEventListener('click', () => {
        const select = document.getElementById('pipeline-add-mode');
        const modeId = select.value;
        const next = SettingsPanelsLogic.appendPipelineMode(
            state.pipeline || [],
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
function bindNotificationEvents() {
    /** DOM の通知設定を読み取りパッチをスケジュールする */
    function applyNotifications() {
        const fieldStates = {};
        for (const [elementId] of SettingsSharedLogic.NOTIFICATION_FIELD_MAP) {
            const element = document.getElementById(elementId);
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
            state.notificationSettings[key] = event.target.checked;
            applyNotifications();
        });
    }
}

/** 履歴・連続記録タブのフォームイベントをバインドする */
function bindHistoryEvents() {
    document.getElementById('history-enabled')?.addEventListener('change', (event) => {
        scheduleApply({ historyEnabled: event.target.checked });
    });
    document.getElementById('record-enabled')?.addEventListener('change', (event) => {
        state.continuousRecordEnabled = event.target.checked;
        scheduleApply({ continuousRecordEnabled: event.target.checked });
        syncContinuousRecordUi();
    });

    const historyLimit = document.getElementById('history-limit');
    if (historyLimit) {
        const bounds = historyLimitBounds();
        /** 履歴保持件数入力をコミットする */
        const commitHistoryLimit = () =>
            commitLimitInput(historyLimit, 'historyLimit', 'historyLimit', bounds);
        historyLimit.addEventListener('change', commitHistoryLimit);
        historyLimit.addEventListener('blur', commitHistoryLimit);
    }

    const recordLimit = document.getElementById('record-limit');
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
            message: '連続記録をすべて削除しますか？\nこの操作は元に戻せません。',
            confirmLabel: '削除する',
            onConfirm: () => postIpcEnvelope({ type: 'clear-record' }),
        });
    });
}

/** 正規表現タブのフォームイベントをバインドする */
function bindRegexEvents() {
    /** DOM の正規表現設定を検証し、有効ならパッチをスケジュールする */
    function applyRegex() {
        const settings = readRegexFromDom();
        const validation = validateRegexSettings(settings);
        updateRegexFeedback();
        if (!validation.valid) {
            return;
        }
        scheduleApply({ regex: settings });
    }

    ['regex-pattern', 'regex-replacement'].forEach((id) => {
        const el = document.getElementById(id);
        el?.addEventListener('input', scheduleRegexFeedback);
        el?.addEventListener('blur', applyRegex);
    });
    ['regex-ci', 'regex-ml'].forEach((id) => {
        document.getElementById(id)?.addEventListener('change', () => {
            scheduleRegexFeedback();
            applyRegex();
        });
    });

    updateRegexFeedback();
}

/** ショートカットタブのイベントをバインドする */
function bindShortcutsEvents() {
    document.getElementById('shortcuts-open-config-btn')?.addEventListener('click', openConfigFile);

    /** DOM からホットキーパッチを組み立てて適用する */
    function applyHotkeysFromDom() {
        const fixedFieldStates = {};
        let valid = true;
        for (const [elementId] of SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP) {
            const element = document.getElementById(elementId);
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

        const favoriteModes = state.favoriteModes || [];
        const favoriteSlotBindings = [];
        for (let index = 0; index < favoriteModes.length; index += 1) {
            const element = document.getElementById('hotkey-favorite-slot-' + index);
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
        state.hotkeys = Object.assign({}, state.hotkeys || {}, hotkeys);
        scheduleApply({ hotkeys });
    }

    document.querySelectorAll('.shortcut-input').forEach((input) => {
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
function bindFavoriteEvents() {
    document.getElementById('favorite-add-btn')?.addEventListener('click', () => {
        const select = document.getElementById('favorite-add-mode');
        const modeId = select.value;
        if (!modeId) {
            return;
        }
        postIpcEnvelope({ type: 'favorite-toggle', mode: modeId });
    });
}

/** 一般タブのイベントをバインドする */
function bindGeneralEvents() {
    document.getElementById('launch-at-login')?.addEventListener('change', (event) => {
        scheduleApply({ launchAtLogin: event.target.checked });
    });
}

/** 登録クリップタブのイベントをバインドする */
function bindClipEvents() {
    document.getElementById('clip-register-btn')?.addEventListener('click', () => {
        if (clipRegisterPending) {
            return;
        }
        setClipRegisterPending(true);
        postIpcEnvelope({ type: 'clip-register' });
    });
}

/** アクティブタブに応じたパネルイベントをバインドする */
function bindActivePanelEvents() {
    switch (activeTab) {
        case 'monitor':
            bindMonitorEvents();
            break;
        case 'notifications':
            bindNotificationEvents();
            break;
        case 'history':
            bindHistoryEvents();
            break;
        case 'regex':
            bindRegexEvents();
            break;
        case 'favorites':
            bindFavoriteEvents();
            break;
        case 'clips':
            bindClipEvents();
            break;
        case 'general':
            bindGeneralEvents();
            break;
        case 'shortcuts':
            bindShortcutsEvents();
            break;
        default:
            break;
    }
}

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
        if (Number.isNaN(index)) {
            return;
        }

        if (button.classList.contains('pipeline-up')) {
            const pipeline = [...state.pipeline];
            if (index <= 0) {
                return;
            }
            [pipeline[index - 1], pipeline[index]] = [pipeline[index], pipeline[index - 1]];
            applyPipelineChange(pipeline);
            return;
        }

        if (button.classList.contains('pipeline-down')) {
            const pipeline = [...state.pipeline];
            if (index >= pipeline.length - 1) {
                return;
            }
            [pipeline[index + 1], pipeline[index]] = [pipeline[index], pipeline[index + 1]];
            applyPipelineChange(pipeline);
            return;
        }

        if (button.classList.contains('pipeline-remove')) {
            applyPipelineChange(state.pipeline.filter((_, i) => i !== index));
        }
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

        if (button.classList.contains('pipeline-up')
            || button.classList.contains('pipeline-down')
            || button.classList.contains('pipeline-remove')) {
            handlePipelineAction(button);
            return;
        }

        if (button.classList.contains('favorite-remove')) {
            postIpcEnvelope({ type: 'favorite-toggle', mode: button.dataset.mode });
            return;
        }

        if (button.classList.contains('favorite-up')) {
            postIpcEnvelope({
                type: 'favorite-move',
                mode: button.dataset.mode,
                direction: 'up',
            });
            return;
        }

        if (button.classList.contains('favorite-down')) {
            postIpcEnvelope({
                type: 'favorite-move',
                mode: button.dataset.mode,
                direction: 'down',
            });
            return;
        }

        if (button.classList.contains('clip-copy')) {
            postIpcEnvelope({ type: 'clip-copy', index: Number(button.dataset.index) });
            return;
        }

        if (button.classList.contains('clip-delete')) {
            showConfirmDialog({
                message: 'この登録クリップを削除しますか？',
                confirmLabel: '削除する',
                onConfirm: () => {
                    postIpcEnvelope({
                        type: 'clip-delete',
                        index: Number(button.dataset.index),
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

        const intervalSelect = document.getElementById('interval-ms');
        if (intervalSelect) {
            intervalSelect.addEventListener('change', () => {
                scheduleApply({ intervalMs: Number(intervalSelect.value) });
            });
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
            if (!modeId) {
                return;
            }
            const pipeline = [...(state.pipeline || [])];
            if (pipeline.length >= maxPipelineLength()) {
                return;
            }
            pipeline.push(modeId);
            applyPipelineChange(pipeline);
        });

        document.getElementById('pipeline-clear-btn')?.addEventListener('click', () => {
            applyPipelineChange([]);
        });
    }

    /** 通知タブのフォームイベントをバインドする */
    function bindNotificationEvents() {
        /** DOM の通知設定を読み取りパッチをスケジュールする */
        function applyNotifications() {
            scheduleApply({
                notificationSettings: {
                    enabled: document.getElementById('notify-enabled').checked,
                    notifyMode: document.getElementById('notify-mode').checked,
                    notifyResult: document.getElementById('notify-result').checked,
                    notifyPause: document.getElementById('notify-pause').checked,
                },
            });
        }

        document.getElementById('notify-enabled')?.addEventListener('change', (event) => {
            state.notificationSettings.enabled = event.target.checked;
            applyNotifications();
            syncNotificationSubPanel();
        });
        ['notify-mode', 'notify-result', 'notify-pause'].forEach((id) => {
            document.getElementById(id)?.addEventListener('change', applyNotifications);
        });
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
            const commitHistoryLimit = () => commitLimitInput(
                historyLimit,
                'historyLimit',
                'historyLimit',
                bounds
            );
            historyLimit.addEventListener('change', commitHistoryLimit);
            historyLimit.addEventListener('blur', commitHistoryLimit);
        }

        const recordLimit = document.getElementById('record-limit');
        if (recordLimit) {
            const bounds = recordLimitBounds();
            /** 連続記録保持件数入力をコミットする */
            const commitRecordLimit = () => commitLimitInput(
                recordLimit,
                'continuousRecordLimit',
                'continuousRecordLimit',
                bounds
            );
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

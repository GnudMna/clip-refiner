    function bindPipelineEvents() {
        document.querySelectorAll('.pipeline-up').forEach((button) => {
            button.addEventListener('click', () => {
                const index = Number(button.dataset.index);
                const pipeline = [...state.pipeline];
                if (index <= 0) {
                    return;
                }
                [pipeline[index - 1], pipeline[index]] = [pipeline[index], pipeline[index - 1]];
                applyPipelineChange(pipeline);
            });
        });

        document.querySelectorAll('.pipeline-down').forEach((button) => {
            button.addEventListener('click', () => {
                const index = Number(button.dataset.index);
                const pipeline = [...state.pipeline];
                if (index >= pipeline.length - 1) {
                    return;
                }
                [pipeline[index + 1], pipeline[index]] = [pipeline[index], pipeline[index + 1]];
                applyPipelineChange(pipeline);
            });
        });

        document.querySelectorAll('.pipeline-remove').forEach((button) => {
            button.addEventListener('click', () => {
                const index = Number(button.dataset.index);
                applyPipelineChange(state.pipeline.filter((_, i) => i !== index));
            });
        });
    }

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

        bindPipelineEvents();
    }

    function bindNotificationEvents() {
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

    function bindHistoryEvents() {
        document.getElementById('history-enabled')?.addEventListener('change', (event) => {
            scheduleApply({ historyEnabled: event.target.checked });
        });
        document.getElementById('record-enabled')?.addEventListener('change', (event) => {
            scheduleApply({ continuousRecordEnabled: event.target.checked });
        });

        const historyLimit = document.getElementById('history-limit');
        if (historyLimit) {
            const bounds = historyLimitBounds();
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

    function bindRegexEvents() {
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

    function bindShortcutsEvents() {
        document.getElementById('shortcuts-open-config-btn')?.addEventListener('click', openConfigFile);
    }

    function bindFavoriteEvents() {
        document.getElementById('favorite-add-btn')?.addEventListener('click', () => {
            const select = document.getElementById('favorite-add-mode');
            const modeId = select.value;
            if (!modeId) {
                return;
            }
            postIpcEnvelope({ type: 'favorite-toggle', mode: modeId });
        });
        document.querySelectorAll('.favorite-remove').forEach((button) => {
            button.addEventListener('click', () => {
                postIpcEnvelope({ type: 'favorite-toggle', mode: button.dataset.mode });
            });
        });
        document.querySelectorAll('.favorite-up').forEach((button) => {
            button.addEventListener('click', () => {
                postIpcEnvelope({
                    type: 'favorite-move',
                    mode: button.dataset.mode,
                    direction: 'up',
                });
            });
        });
        document.querySelectorAll('.favorite-down').forEach((button) => {
            button.addEventListener('click', () => {
                postIpcEnvelope({
                    type: 'favorite-move',
                    mode: button.dataset.mode,
                    direction: 'down',
                });
            });
        });
    }

    function bindGeneralEvents() {
        document.getElementById('launch-at-login')?.addEventListener('change', (event) => {
            scheduleApply({ launchAtLogin: event.target.checked });
        });
    }

    function bindClipEvents() {
        document.getElementById('clip-register-btn')?.addEventListener('click', () => {
            if (clipRegisterPending) {
                return;
            }
            setClipRegisterPending(true);
            postIpcEnvelope({ type: 'clip-register' });
        });
        document.querySelectorAll('.clip-copy').forEach((button) => {
            button.addEventListener('click', () => {
                postIpcEnvelope({ type: 'clip-copy', index: Number(button.dataset.index) });
            });
        });
        document.querySelectorAll('.clip-delete').forEach((button) => {
            button.addEventListener('click', () => {
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
            });
        });
    }

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

    function renderActivePanel() {
        const content = document.getElementById('content');
        const renderer = PANEL_RENDERERS[activeTab] || PANEL_RENDERERS.general;
        content.innerHTML = renderer();
        bindActivePanelEvents();
        content.scrollTop = tabScrollPositions[activeTab] || 0;
    }

    function render() {
        renderSidebar();
        renderActivePanel();
    }

    function shouldFullRenderOnLoad(options, wasEditing) {
        if (options.force || options.externalSync) {
            return true;
        }
        if (clipRegisterPending && activeTab === 'clips') {
            return true;
        }
        if (!wasEditing) {
            return true;
        }
        return activeTab === 'favorites' || activeTab === 'shortcuts';
    }

    window.loadSettings = function loadSettings(snapshot, options) {
        const opts = options || {};
        const wasEditing = isContentEditing();
        const editingTab = wasEditing ? activeTab : null;

        if (editingTab === 'regex' && !opts.externalSync) {
            snapshot = Object.assign({}, snapshot, {
                regex: readRegexFromDom(),
            });
        }

        state = snapshot;

        if (opts.externalSync) {
            showApplyFeedback({
                ok: true,
                message: '設定ファイルが外部で更新されました',
            });
        }

        if (clipRegisterPending && activeTab !== 'clips') {
            clipRegisterPending = false;
            renderSidebar();
            syncServerFields();
            return;
        }

        if (shouldFullRenderOnLoad(opts, wasEditing)) {
            clipRegisterPending = false;
            render();
            return;
        }

        clipRegisterPending = false;
        renderSidebar();
        syncServerFields();
    };

    window.updateClips = function updateClips(clips) {
        if (!state) {
            return;
        }
        state.clips = clips;
        clipRegisterPending = false;
        if (activeTab === 'clips') {
            renderActivePanel();
        }
    };

    window.updateFavoritesAndShortcuts = function updateFavoritesAndShortcuts(payload) {
        if (!state) {
            return;
        }
        state.favoriteModes = payload.favoriteModes || [];
        state.shortcuts = payload.shortcuts || [];
        if (activeTab === 'favorites' || activeTab === 'shortcuts') {
            renderActivePanel();
        }
    };

    window.flushPendingApply = flushPendingApply;
    window.onApplyFeedback = showApplyFeedback;
    window.onRegexPreview = handleRegexPreview;

    bindConfirmDialog();
    document.getElementById('open-config-btn').addEventListener('click', openConfigFile);
    document.getElementById('close-btn').addEventListener('click', closeWindow);

    document.addEventListener('keydown', (event) => {
        if (event.key === 'Escape') {
            const dialog = document.getElementById('confirm-dialog');
            if (dialog && !dialog.hidden) {
                return;
            }
            if (isContentEditing()) {
                return;
            }
            closeWindow();
        }
    });

    const TABS = [
        {
            id: 'general',
            label: '一般',
            description: '起動時の動作などアプリ全体の設定を管理する',
        },
        {
            id: 'favorites',
            label: 'お気に入り',
            description: 'クイックセレクターとホットキーで使う変換モードを登録する',
        },
        {
            id: 'regex',
            label: '正規表現',
            description: '正規表現モード用のパターンと置換文字列を編集する',
        },
        {
            id: 'history',
            label: '履歴・連続記録',
            description: 'クリップボード履歴と連続記録の保持設定を管理する',
        },
        {
            id: 'clips',
            label: '登録クリップ',
            description: 'よく使うテキストや画像を登録してワンタッチでコピーする',
        },
        {
            id: 'monitor',
            label: '監視',
            description: 'クリップボード監視の方式・周期・加工パイプラインを設定する',
        },
        {
            id: 'notifications',
            label: '通知',
            description: 'デスクトップ通知の表示条件を設定する',
        },
        {
            id: 'shortcuts',
            label: 'ショートカット',
            description: 'グローバルホットキーの一覧を確認する',
        },
    ];

    const INTERVAL_OPTIONS = [
        { value: 500, label: '0.5秒' },
        { value: 1000, label: '1秒' },
        { value: 2000, label: '2秒' },
        { value: 5000, label: '5秒' },
    ];

    const REGEX_SAMPLE_TEXT = 'Hello world 123';

    let state = null;
    let activeTab = 'general';
    let applyTimer = null;
    let applyFeedbackTimer = null;
    let pendingPatch = {};
    let clipRegisterPending = false;
    let regexFeedbackTimer = null;
    let regexPreviewRequestId = 0;
    let latestRegexPreviewId = 0;
    let pendingRegexPreviewRequest = null;
    let lastRegexPreview = { settings: null, valid: true, message: '' };
    let confirmDialogOnConfirm = null;
    const tabScrollPositions = {};

    function limits() {
        return state?.limits ?? {};
    }

    function historyLimitBounds() {
        const l = limits();
        return {
            min: l.historyMin ?? 1,
            max: l.historyMax ?? 100,
        };
    }

    function recordLimitBounds() {
        const l = limits();
        return {
            min: l.continuousRecordMin ?? 1,
            max: l.continuousRecordMax ?? 100,
        };
    }

    function maxPipelineLength() {
        return limits().maxPipelineLength ?? 10;
    }

    function maxFavoriteModes() {
        return limits().maxFavoriteModes ?? 20;
    }

    function maxRegisteredClips() {
        return limits().maxRegisteredClips ?? 100;
    }

    function maxRegexPatternBytes() {
        return limits().maxRegexPatternBytes ?? 8 * 1024;
    }

    function postIpc(message) {
        window.ipc.postMessage(message);
    }

    function postIpcEnvelope(envelope) {
        postIpc(JSON.stringify(envelope));
    }

    function showApplyFeedback(feedback) {
        const el = document.getElementById('apply-feedback');
        if (!el) {
            return;
        }
        clearTimeout(applyFeedbackTimer);
        const ok = feedback?.ok !== false;
        el.hidden = false;
        el.textContent = feedback?.message
            || (ok ? '設定を保存しました' : '設定の適用に失敗しました');
        el.className = 'apply-feedback apply-feedback--' + (ok ? 'ok' : 'error');
        applyFeedbackTimer = setTimeout(() => {
            el.hidden = true;
        }, ok ? 2500 : 6000);
    }

    function stripInvalidPendingRegex() {
        if (pendingPatch.regex === undefined) {
            return;
        }
        const validation = validateRegexSettings(pendingPatch.regex);
        if (validation.valid) {
            return;
        }
        delete pendingPatch.regex;
        showApplyFeedback({
            ok: false,
            field: 'regex',
            message: validation.message,
        });
    }

    function flushPendingApply() {
        if (applyTimer !== null) {
            clearTimeout(applyTimer);
            applyTimer = null;
        }
        stripInvalidPendingRegex();
        if (Object.keys(pendingPatch).length > 0) {
            postIpcEnvelope({ type: 'apply', patch: pendingPatch });
            pendingPatch = {};
        }
    }

    function closeWindow() {
        flushPendingApply();
        postIpcEnvelope({ type: 'close' });
    }

    function openConfigFile() {
        postIpcEnvelope({ type: 'open-config' });
    }

    function showConfirmDialog(options) {
        const dialog = document.getElementById('confirm-dialog');
        const messageEl = document.getElementById('confirm-dialog-message');
        const okBtn = document.getElementById('confirm-dialog-ok');
        if (!dialog || !messageEl || !okBtn) {
            return;
        }

        messageEl.textContent = options.message;
        okBtn.textContent = options.confirmLabel || '確認';
        confirmDialogOnConfirm = options.onConfirm || null;
        dialog.hidden = false;
        okBtn.focus();
    }

    function hideConfirmDialog() {
        const dialog = document.getElementById('confirm-dialog');
        if (dialog) {
            dialog.hidden = true;
        }
        confirmDialogOnConfirm = null;
    }

    function bindConfirmDialog() {
        const dialog = document.getElementById('confirm-dialog');
        const okBtn = document.getElementById('confirm-dialog-ok');
        if (!dialog || !okBtn) {
            return;
        }

        okBtn.addEventListener('click', () => {
            const onConfirm = confirmDialogOnConfirm;
            hideConfirmDialog();
            onConfirm?.();
        });

        dialog.querySelectorAll('[data-confirm-cancel]').forEach((el) => {
            el.addEventListener('click', hideConfirmDialog);
        });

        document.addEventListener('keydown', (event) => {
            if (dialog.hidden) {
                return;
            }
            if (event.key === 'Escape') {
                event.preventDefault();
                hideConfirmDialog();
            }
        });
    }

    function mergePatch(target, patch) {
        for (const [key, value] of Object.entries(patch)) {
            if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
                const nested = target[key] && typeof target[key] === 'object' && !Array.isArray(target[key])
                    ? target[key]
                    : {};
                target[key] = mergePatch(nested, value);
            } else {
                target[key] = value;
            }
        }
        return target;
    }

    function scheduleApply(patch) {
        mergePatch(pendingPatch, patch);
        clearTimeout(applyTimer);
        applyTimer = setTimeout(() => {
            stripInvalidPendingRegex();
            if (Object.keys(pendingPatch).length === 0) {
                applyTimer = null;
                return;
            }
            postIpcEnvelope({ type: 'apply', patch: pendingPatch });
            pendingPatch = {};
            applyTimer = null;
        }, 200);
    }

    function isContentEditing() {
        const el = document.activeElement;
        const content = document.getElementById('content');
        if (!el || !content?.contains(el)) {
            return false;
        }
        const tag = el.tagName;
        return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    }

    function readRegexFromDom() {
        return {
            pattern: document.getElementById('regex-pattern')?.value ?? state.regex.pattern,
            replacement: document.getElementById('regex-replacement')?.value ?? state.regex.replacement,
            caseInsensitive: document.getElementById('regex-ci')?.checked ?? state.regex.caseInsensitive,
            multiline: document.getElementById('regex-ml')?.checked ?? state.regex.multiline,
        };
    }

    function validateRegexSettings(settings) {
        return RegexPreview.validateRegexSettings(
            settings,
            lastRegexPreview,
            maxRegexPatternBytes()
        );
    }

    function requestRegexPreview(settings) {
        const requestId = ++regexPreviewRequestId;
        latestRegexPreviewId = requestId;
        pendingRegexPreviewRequest = {
            requestId,
            settings: RegexPreview.copyRegexSettings(settings),
        };
        postIpcEnvelope({
            type: 'regex-preview',
            requestId,
            settings: {
                pattern: settings.pattern,
                replacement: settings.replacement,
                caseInsensitive: settings.caseInsensitive,
                multiline: settings.multiline,
            },
        });

        const previewEl = document.getElementById('regex-preview-result');
        if (previewEl) {
            previewEl.textContent = '…';
            previewEl.classList.remove('regex-preview-result--error');
        }
    }

    function handleRegexPreview(payload) {
        const statusEl = document.getElementById('regex-status');
        const previewEl = document.getElementById('regex-preview-result');
        const patternField = document.getElementById('regex-pattern')?.closest('.field');
        if (!statusEl || !previewEl) {
            return;
        }

        const result = RegexPreview.resolveRegexPreviewResponse({
            payload,
            latestRequestId: latestRegexPreviewId,
            pendingRequest: pendingRegexPreviewRequest,
            domSettings: readRegexFromDom(),
        });

        if (result.action === 'ignore') {
            return;
        }
        if (result.action === 'stale') {
            scheduleRegexFeedback();
            return;
        }

        lastRegexPreview = result.lastRegexPreview;
        const ui = result.ui;
        statusEl.textContent = ui.statusMessage;
        statusEl.className = ui.statusClass;

        if (patternField) {
            patternField.classList.toggle('field-invalid', ui.fieldInvalid);
        }

        previewEl.textContent = ui.previewText;
        previewEl.classList.toggle('regex-preview-result--error', ui.previewError);
    }

    function updateRegexFeedback() {
        requestRegexPreview(readRegexFromDom());
    }

    function scheduleRegexFeedback() {
        clearTimeout(regexFeedbackTimer);
        regexFeedbackTimer = setTimeout(updateRegexFeedback, 150);
    }

    function clampLimitValue(raw, min, max) {
        let value = Number.parseInt(String(raw), 10);
        if (Number.isNaN(value)) {
            value = min;
        }
        const clamped = Math.min(max, Math.max(min, value));
        return {
            value: clamped,
            clamped: clamped !== value,
        };
    }

    function showFieldFeedback(input, message) {
        const field = input.closest('.field');
        if (!field) {
            return;
        }
        let feedback = field.querySelector('.field-feedback');
        if (!feedback) {
            feedback = document.createElement('p');
            feedback.className = 'field-feedback';
            field.appendChild(feedback);
        }
        feedback.textContent = message;
        feedback.hidden = false;
        field.classList.add('field-adjusted');
    }

    function clearFieldFeedback(input) {
        const field = input.closest('.field');
        if (!field) {
            return;
        }
        const feedback = field.querySelector('.field-feedback');
        if (feedback) {
            feedback.hidden = true;
        }
        field.classList.remove('field-adjusted');
    }

    function commitLimitInput(input, patchKey, stateKey, bounds) {
        const result = clampLimitValue(input.value, bounds.min, bounds.max);
        input.value = String(result.value);
        if (result.clamped) {
            showFieldFeedback(
                input,
                bounds.min + '〜' + bounds.max + ' の範囲に調整しました'
            );
        } else {
            clearFieldFeedback(input);
        }
        state[stateKey] = result.value;
        scheduleApply({ [patchKey]: result.value });
    }

    function modeProducesImage(modeId) {
        const mode = state.modes.find((entry) => entry.id === modeId);
        return mode?.producesImage === true;
    }

    function normalizePipeline(pipeline) {
        let imageMode = null;
        for (let index = pipeline.length - 1; index >= 0; index -= 1) {
            if (modeProducesImage(pipeline[index])) {
                imageMode = pipeline[index];
                break;
            }
        }
        let normalized = pipeline.filter((modeId) => !modeProducesImage(modeId));
        if (imageMode !== null) {
            normalized.push(imageMode);
        }
        const maxLen = maxPipelineLength();
        if (normalized.length > maxLen) {
            normalized = normalized.slice(0, maxLen);
        }
        return normalized;
    }

    function applyPipelineChange(pipeline) {
        const normalized = normalizePipeline(pipeline);
        state.pipeline = normalized;
        scheduleApply({ pipeline: normalized });
        updateMonitorPipelineUi();
    }

    function syncMonitorPauseBanner() {
        const banner = document.getElementById('monitor-pause-banner');
        if (banner) {
            banner.hidden = !state.isPaused;
        }
    }

    function syncMonitorIntervalSection() {
        const section = document.getElementById('interval-section');
        if (section) {
            section.classList.toggle('disabled', state.monitorMode === 'Event');
        }
    }

    function buildPipelineInnerHtml() {
        const pipeline = state.pipeline || [];
        if (pipeline.length === 0) {
            return '<div class="empty-hint">パイプラインは未設定です (デフォルトモードのみ適用)</div>';
        }
        return '<div class="pipeline-list">' + pipeline.map((modeId, index) => {
            return '<div class="pipeline-item" data-index="' + index + '">'
                + '<span class="pipeline-step">' + (index + 1) + '</span>'
                + '<span class="label">' + escapeHtml(modeLabel(modeId)) + '</span>'
                + '<div class="pipeline-actions">'
                + '<button type="button" class="icon-btn pipeline-up" data-index="' + index + '" title="上へ">↑</button>'
                + '<button type="button" class="icon-btn pipeline-down" data-index="' + index + '" title="下へ">↓</button>'
                + '<button type="button" class="icon-btn pipeline-remove" data-index="' + index + '" title="削除">×</button>'
                + '</div></div>';
        }).join('') + '</div>';
    }

    function updateMonitorPipelineUi() {
        const container = document.getElementById('pipeline-list-container');
        if (!container) {
            return;
        }
        container.innerHTML = buildPipelineInnerHtml();
        bindPipelineEvents();
    }

    function syncNotificationSubPanel() {
        const sub = document.querySelector('.notify-sub');
        if (sub) {
            sub.classList.toggle('is-disabled', !state.notificationSettings.enabled);
        }
    }

    function syncServerFields() {
        const launch = document.getElementById('launch-at-login');
        if (launch) {
            launch.checked = state.launchAtLogin;
        }
        const paused = document.getElementById('monitor-paused');
        if (paused) {
            paused.checked = state.isPaused;
        }
        syncMonitorPauseBanner();
        syncMonitorIntervalSection();
        syncNotificationSubPanel();
        updateClipRegisterButton();
    }

    function setClipRegisterPending(pending) {
        clipRegisterPending = pending;
        updateClipRegisterButton();
    }

    function updateClipRegisterButton() {
        const btn = document.getElementById('clip-register-btn');
        if (!btn) {
            return;
        }
        btn.disabled = clipRegisterPending;
        btn.textContent = clipRegisterPending ? '登録中...' : 'クリップボードから登録';
        btn.classList.toggle('is-loading', clipRegisterPending);
    }

    function escapeHtml(text) {
        return String(text)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function tabMeta(tabId) {
        return TABS.find((tab) => tab.id === tabId) || TABS[0];
    }

    function panelWrap(tabId, bodyHtml) {
        const tab = tabMeta(tabId);
        return ''
            + '<div class="panel active" id="panel-' + tabId + '">'
            + '<header class="panel-intro">'
            + '<h2 class="panel-title">' + escapeHtml(tab.label) + '</h2>'
            + '<p class="panel-desc">' + escapeHtml(tab.description) + '</p>'
            + '</header>'
            + bodyHtml
            + '</div>';
    }

    function card(title, bodyHtml) {
        return ''
            + '<section class="card">'
            + '<h3 class="card-title">' + escapeHtml(title) + '</h3>'
            + '<div class="card-body">' + bodyHtml + '</div>'
            + '</section>';
    }

    function optionCheckbox(id, checked, label, description) {
        const descHtml = description
            ? '<span class="option-desc">' + escapeHtml(description) + '</span>'
            : '';
        return ''
            + '<label class="option-card">'
            + '<input type="checkbox" id="' + escapeHtml(id) + '"' + (checked ? ' checked' : '') + '>'
            + '<span class="option-text">'
            + '<span class="option-label">' + escapeHtml(label) + '</span>'
            + descHtml
            + '</span></label>';
    }

    function wrappedTable(tableHtml) {
        return '<div class="list-table-wrap">' + tableHtml + '</div>';
    }

    function modeLabel(modeId) {
        const mode = state.modes.find((entry) => entry.id === modeId);
        return mode ? mode.label : modeId;
    }

    function buildModeOptions(selectedId) {
        const categories = [];
        for (const mode of state.modes) {
            if (!categories.includes(mode.category)) {
                categories.push(mode.category);
            }
        }
        let html = '';
        for (const category of categories) {
            html += '<optgroup label="' + escapeHtml(category) + '">';
            for (const mode of state.modes.filter((entry) => entry.category === category)) {
                const selected = mode.id === selectedId ? ' selected' : '';
                html += '<option value="' + escapeHtml(mode.id) + '"' + selected + '>'
                    + escapeHtml(mode.label) + '</option>';
            }
            html += '</optgroup>';
        }
        return html;
    }

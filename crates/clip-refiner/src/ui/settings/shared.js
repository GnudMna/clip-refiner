    // ======================================================================
    // 設定画面 UI 共有状態・ユーティリティ
    // ======================================================================

    /** @type {Array<{ id: string, label: string, description: string }>} */
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

    /** @type {Array<{ value: number, label: string }>} */
    const INTERVAL_OPTIONS = [
        { value: 500, label: '0.5秒' },
        { value: 1000, label: '1秒' },
        { value: 2000, label: '2秒' },
        { value: 5000, label: '5秒' },
    ];

    /** @type {Record<string, unknown>|null} Rust から受信した設定スナップショット */
    let state = null;
    /** @type {string} 現在表示中のタブ ID */
    let activeTab = 'general';
    /** @type {ReturnType<typeof setTimeout>|null} 設定パッチ送信のデバウンスタイマー */
    let applyTimer = null;
    /** @type {ReturnType<typeof setTimeout>|null} 適用フィードバック非表示タイマー */
    let applyFeedbackTimer = null;
    /** @type {Record<string, unknown>} デバウンス中の設定パッチ */
    let pendingPatch = {};
    /** @type {boolean} クリップボードからの登録処理が進行中か */
    let clipRegisterPending = false;
    /** @type {ReturnType<typeof setTimeout>|null} 正規表現プレビュー更新のデバウンスタイマー */
    let regexFeedbackTimer = null;
    /** @type {number} 正規表現プレビュー IPC リクエスト ID (単調増加) */
    let regexPreviewRequestId = 0;
    /** @type {number} 最新の正規表現プレビュー リクエスト ID */
    let latestRegexPreviewId = 0;
    /**
     * @type {{ requestId: number, settings: import('./regex_preview').RegexSettings }|null}
     * 送信中の正規表現プレビューリクエスト
     */
    let pendingRegexPreviewRequest = null;
    /** @type {import('./regex_preview').RegexPreviewCache} 直近の正規表現プレビュー検証結果 */
    let lastRegexPreview = { settings: null, valid: true, message: '' };
    /** @type {(() => void)|null} 確認ダイアログ OK 押下時のコールバック */
    let confirmDialogOnConfirm = null;
    /** @type {Record<string, number>} タブごとのコンテンツスクロール位置 */
    const tabScrollPositions = {};
    /** @type {Set<string>} HTML 生成済みのタブ ID */
    const renderedPanels = new Set();
    /** @type {Set<string>} イベントバインド済みのタブ ID */
    const boundPanels = new Set();
    /** @type {boolean} サイドバーが初期化済みか */
    let sidebarInitialized = false;
    /** @type {boolean} コンテンツ領域のクリック委譲がバインド済みか */
    let contentDelegationBound = false;

    /**
     * サーバーから渡された制限値オブジェクトを返す
     *
     * @returns {Record<string, number>}
     */
    function limits() {
        return state?.limits ?? {};
    }

    /**
     * 履歴保持件数の min/max
     *
     * @returns {{ min: number, max: number }}
     */
    function historyLimitBounds() {
        const l = limits();
        return {
            min: l.historyMin ?? 1,
            max: l.historyMax ?? 100,
        };
    }

    /**
     * 連続記録保持件数の min/max
     *
     * @returns {{ min: number, max: number }}
     */
    function recordLimitBounds() {
        const l = limits();
        return {
            min: l.continuousRecordMin ?? 1,
            max: l.continuousRecordMax ?? 100,
        };
    }

    /** @returns {number} 加工パイプラインの最大段数 */
    function maxPipelineLength() {
        return limits().maxPipelineLength ?? 10;
    }

    /** @returns {number} お気に入りモードの最大件数 */
    function maxFavoriteModes() {
        return limits().maxFavoriteModes ?? 20;
    }

    /** @returns {number} 登録クリップの最大件数 */
    function maxRegisteredClips() {
        return limits().maxRegisteredClips ?? 100;
    }

    /** @returns {number} 正規表現パターンの最大バイト数 */
    function maxRegexPatternBytes() {
        return limits().maxRegexPatternBytes ?? 8 * 1024;
    }

    /**
     * WebView IPC に生メッセージを送信する
     *
     * @param {string} message
     */
    function postIpc(message) {
        window.ipc.postMessage(message);
    }

    /**
     * JSON エンベロープを IPC 送信する
     *
     * @param {Record<string, unknown>} envelope
     */
    function postIpcEnvelope(envelope) {
        postIpc(JSON.stringify(envelope));
    }

    /**
     * 設定適用のフィードバックバナーを表示する
     *
     * @param {{ ok?: boolean, message?: string, field?: string }} [feedback]
     */
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

    /**
     * 保留中パッチの無効な正規表現設定を除去する
     */
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

    /**
     * デバウンス中の設定パッチを即時送信する
     */
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

    /** 保留パッチをフラッシュして設定ウィンドウを閉じる */
    function closeWindow() {
        flushPendingApply();
        postIpcEnvelope({ type: 'close' });
    }

    /** `config.toml` を OS の既定エディタで開く */
    function openConfigFile() {
        postIpcEnvelope({ type: 'open-config' });
    }

    /**
     * 確認ダイアログを表示する
     *
     * @param {{ message: string, confirmLabel?: string, onConfirm?: () => void }} options
     */
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

    /** 確認ダイアログを非表示にする */
    function hideConfirmDialog() {
        const dialog = document.getElementById('confirm-dialog');
        if (dialog) {
            dialog.hidden = true;
        }
        confirmDialogOnConfirm = null;
    }

    /** 確認ダイアログのボタン・Escape キーをバインドする */
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

    /**
     * `SettingsPatch.mergePatch` のエイリアス
     *
     * @param {Record<string, unknown>} target
     * @param {Record<string, unknown>} patch
     * @returns {Record<string, unknown>}
     */
    function mergePatch(target, patch) {
        return SettingsPatch.mergePatch(target, patch);
    }

    /**
     * 設定パッチをデバウンスして IPC 送信する
     *
     * @param {Record<string, unknown>} patch
     */
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

    /**
     * コンテンツ領域でフォーム要素が編集中かどうか
     *
     * @returns {boolean}
     */
    function isContentEditing() {
        const el = document.activeElement;
        const content = document.getElementById('content');
        if (!el || !content?.contains(el)) {
            return false;
        }
        const tag = el.tagName;
        return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    }

    /**
     * 正規表現タブの DOM から現在の設定を読み取る
     *
     * @returns {import('./regex_preview').RegexSettings}
     */
    function readRegexFromDom() {
        return {
            pattern: document.getElementById('regex-pattern')?.value ?? state.regex.pattern,
            replacement: document.getElementById('regex-replacement')?.value ?? state.regex.replacement,
            caseInsensitive: document.getElementById('regex-ci')?.checked ?? state.regex.caseInsensitive,
            multiline: document.getElementById('regex-ml')?.checked ?? state.regex.multiline,
        };
    }

    /**
     * 正規表現設定を検証する
     *
     * @param {import('./regex_preview').RegexSettings} settings
     * @returns {import('./regex_preview').RegexValidationResult}
     */
    function validateRegexSettings(settings) {
        return RegexPreview.validateRegexSettings(
            settings,
            lastRegexPreview,
            maxRegexPatternBytes()
        );
    }

    /**
     * Rust 側に正規表現プレビューをリクエストする
     *
     * @param {import('./regex_preview').RegexSettings} settings
     */
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

    /**
     * 正規表現プレビュー IPC 応答を処理する
     *
     * @param {import('./regex_preview').RegexPreviewPayload} payload
     */
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

    /** DOM の正規表現設定でプレビューを即時更新する */
    function updateRegexFeedback() {
        requestRegexPreview(readRegexFromDom());
    }

    /** 正規表現プレビュー更新をデバウンスする */
    function scheduleRegexFeedback() {
        clearTimeout(regexFeedbackTimer);
        regexFeedbackTimer = setTimeout(updateRegexFeedback, 150);
    }

    /**
     * 数値入力を min/max の範囲にクランプする
     *
     * @param {string|number} raw
     * @param {number} min
     * @param {number} max
     * @returns {{ value: number, clamped: boolean }}
     */
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

    /**
     * フィールドに調整フィードバックを表示する
     *
     * @param {HTMLElement} input
     * @param {string} message
     */
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

    /**
     * フィールドの調整フィードバックを消す
     *
     * @param {HTMLElement} input
     */
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

    /**
     * 件数入力をコミットし state とパッチを更新する
     *
     * @param {HTMLInputElement} input
     * @param {string} patchKey - `scheduleApply` に渡すキー
     * @param {string} stateKey - `state` 上のキー
     * @param {{ min: number, max: number }} bounds
     */
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

    /**
     * 指定モードが画像を出力するかどうか
     *
     * @param {string} modeId
     * @returns {boolean}
     */
    function modeProducesImage(modeId) {
        const mode = state.modes.find((entry) => entry.id === modeId);
        return mode?.producesImage === true;
    }

    /**
     * 加工パイプラインを正規化する
     *
     * 画像出力モードは末尾に1つのみ残し、最大段数を超える分は切り捨てる
     *
     * @param {string[]} pipeline
     * @returns {string[]}
     */
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

    /**
     * パイプライン変更を正規化・適用し UI を更新する
     *
     * @param {string[]} pipeline
     */
    function applyPipelineChange(pipeline) {
        const normalized = normalizePipeline(pipeline);
        state.pipeline = normalized;
        scheduleApply({ pipeline: normalized });
        updateMonitorPipelineUi();
    }

    /** 監視一時停止バナーの表示状態を同期する */
    function syncMonitorPauseBanner() {
        const banner = document.getElementById('monitor-pause-banner');
        if (banner) {
            banner.hidden = !state.isPaused;
        }
    }

    /** 監視タブの連続記録バナー表示を同期する */
    function syncMonitorContinuousRecordBanner() {
        const banner = document.getElementById('monitor-record-banner');
        if (banner) {
            banner.hidden = !state.continuousRecordEnabled;
        }
    }

    /** 連続記録中は加工パイプラインセクションを無効表示にする */
    function syncMonitorProcessingSection() {
        const section = document.getElementById('monitor-processing-section');
        if (section) {
            section.classList.toggle('is-disabled', state.continuousRecordEnabled);
        }
    }

    /** 履歴タブの連続記録バナー表示を同期する */
    function syncRecordActiveBanner() {
        const banner = document.getElementById('record-active-banner');
        if (banner) {
            banner.hidden = !state.continuousRecordEnabled;
        }
    }

    /** 連続記録関連 UI を一括同期する */
    function syncContinuousRecordUi() {
        syncMonitorContinuousRecordBanner();
        syncMonitorProcessingSection();
        syncRecordActiveBanner();
    }

    /** イベント監視時はポーリング間隔セクションを無効表示にする */
    function syncMonitorIntervalSection() {
        const section = document.getElementById('interval-section');
        if (section) {
            section.classList.toggle('disabled', state.monitorMode === 'Event');
        }
    }

    /**
     * 加工パイプライン一覧の内部 HTML を組み立てる
     *
     * @returns {string}
     */
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

    /** 加工パイプライン一覧 DOM を再描画する */
    function updateMonitorPipelineUi() {
        const container = document.getElementById('pipeline-list-container');
        if (!container) {
            return;
        }
        container.innerHTML = buildPipelineInnerHtml();
    }

    /**
     * 指定タブのパネルをキャッシュから破棄する
     *
     * @param {string} tabId
     */
    function invalidatePanel(tabId) {
        renderedPanels.delete(tabId);
        boundPanels.delete(tabId);
        document.getElementById('panel-' + tabId)?.remove();
    }

    /** 全タブのパネルキャッシュを破棄しコンテンツを空にする */
    function invalidateAllPanels() {
        renderedPanels.clear();
        boundPanels.clear();
        const content = document.getElementById('content');
        if (content) {
            content.innerHTML = '';
        }
    }

    /** アクティブタブのパネルを表示しスクロール位置を復元する */
    function showActivePanel() {
        const content = document.getElementById('content');
        if (!content) {
            return;
        }
        content.querySelectorAll('.panel').forEach((panel) => {
            const isActive = panel.id === 'panel-' + activeTab;
            panel.classList.toggle('active', isActive);
        });
        content.scrollTop = tabScrollPositions[activeTab] || 0;
    }

    /** 通知マスター OFF 時は個別通知設定を無効表示にする */
    function syncNotificationSubPanel() {
        const sub = document.querySelector('.notify-sub');
        if (sub) {
            sub.classList.toggle('is-disabled', !state.notificationSettings.enabled);
        }
    }

    /** サーバー主導のフィールド (一時停止・連続記録など) を DOM に反映する */
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
        syncContinuousRecordUi();
        syncMonitorIntervalSection();
        syncNotificationSubPanel();
        updateClipRegisterButton();
    }

    /**
     * クリップ登録ボタンのローディング状態を設定する
     *
     * @param {boolean} pending
     */
    function setClipRegisterPending(pending) {
        clipRegisterPending = pending;
        updateClipRegisterButton();
    }

    /** クリップ登録ボタンの表示を `clipRegisterPending` に合わせて更新する */
    function updateClipRegisterButton() {
        const btn = document.getElementById('clip-register-btn');
        if (!btn) {
            return;
        }
        btn.disabled = clipRegisterPending;
        btn.textContent = clipRegisterPending ? '登録中...' : 'クリップボードから登録';
        btn.classList.toggle('is-loading', clipRegisterPending);
    }

    /**
     * HTML 特殊文字をエスケープする
     *
     * @param {string} text
     * @returns {string}
     */
    function escapeHtml(text) {
        return String(text)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    /**
     * タブ ID からタブメタデータを取得する
     *
     * @param {string} tabId
     * @returns {{ id: string, label: string, description: string }}
     */
    function tabMeta(tabId) {
        return TABS.find((tab) => tab.id === tabId) || TABS[0];
    }

    /**
     * パネル外枠 HTML を組み立てる
     *
     * @param {string} tabId
     * @param {string} bodyHtml
     * @returns {string}
     */
    function panelWrap(tabId, bodyHtml) {
        const tab = tabMeta(tabId);
        const activeClass = tabId === activeTab ? ' active' : '';
        return ''
            + '<div class="panel' + activeClass + '" id="panel-' + tabId + '">'
            + '<header class="panel-intro">'
            + '<h2 class="panel-title">' + escapeHtml(tab.label) + '</h2>'
            + '<p class="panel-desc">' + escapeHtml(tab.description) + '</p>'
            + '</header>'
            + bodyHtml
            + '</div>';
    }

    /**
     * カードセクション HTML を組み立てる
     *
     * @param {string} title
     * @param {string} bodyHtml
     * @returns {string}
     */
    function card(title, bodyHtml) {
        return ''
            + '<section class="card">'
            + '<h3 class="card-title">' + escapeHtml(title) + '</h3>'
            + '<div class="card-body">' + bodyHtml + '</div>'
            + '</section>';
    }

    /**
     * オプションカード形式のチェックボックス HTML を組み立てる
     *
     * @param {string} id
     * @param {boolean} checked
     * @param {string} label
     * @param {string} [description]
     * @returns {string}
     */
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

    /**
     * テーブルを横スクロール用ラッパーで包む
     *
     * @param {string} tableHtml
     * @returns {string}
     */
    function wrappedTable(tableHtml) {
        return '<div class="list-table-wrap">' + tableHtml + '</div>';
    }

    /**
     * モード ID から表示ラベルを取得する
     *
     * @param {string} modeId
     * @returns {string}
     */
    function modeLabel(modeId) {
        const mode = state.modes.find((entry) => entry.id === modeId);
        return mode ? mode.label : modeId;
    }

    /**
     * カテゴリ別 `<optgroup>` 付きモード選択 `<option>` HTML を組み立てる
     *
     * @param {string} selectedId
     * @returns {string}
     */
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

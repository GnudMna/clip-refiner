// ======================================================================
// 設定画面 パネル HTML 生成
// ======================================================================

/** @type {Record<string, () => string>} */
const PANEL_RENDERERS = {
    general: renderGeneralPanel,
    favorites: renderFavoritesPanel,
    regex: renderRegexPanel,
    history: renderHistoryPanel,
    clips: renderClipsPanel,
    monitor: renderMonitorPanel,
    notifications: renderNotificationsPanel,
    shortcuts: renderShortcutsPanel,
};

/** サイドバーのアクティブタブ表示を DOM に反映する */
function syncSidebarActiveTab() {
    const sidebar = document.getElementById('sidebar');
    if (!sidebar) {
        return;
    }
    sidebar.querySelectorAll('.nav-btn').forEach((button) => {
        const isActive = button.dataset.tab === activeTab;
        button.classList.toggle('active', isActive);
        if (isActive) {
            button.setAttribute('aria-current', 'page');
        } else {
            button.removeAttribute('aria-current');
        }
    });
}

/** サイドバーナビゲーションの内部 HTML を組み立てる */
function buildSidebarNavInnerHtml() {
    return SettingsPanelsLogic.buildSidebarNavInnerHtml(TABS, activeTab, escapeHtml);
}

/**
 * サイドバーを初期化またはアクティブ状態を更新する
 *
 * タブ切替前に現在タブのスクロール位置を保存する
 */
function ensureSidebar() {
    const content = document.getElementById('content');
    if (content) {
        tabScrollPositions[activeTab] = content.scrollTop;
    }

    const sidebar = document.getElementById('sidebar');
    if (!sidebar) {
        return;
    }

    if (!sidebarInitialized) {
        sidebar.innerHTML = buildSidebarNavInnerHtml();
        sidebar.addEventListener('click', (event) => {
            const button = event.target.closest('.nav-btn');
            if (!button || button.dataset.tab === activeTab) {
                return;
            }
            activeTab = button.dataset.tab;
            render();
        });
        sidebarInitialized = true;
        return;
    }

    syncSidebarActiveTab();
}

/** @returns {string} 監視タブのパネル HTML */
function renderMonitorPanel() {
    const intervalDisabled = state.monitorMode === 'Event' ? ' disabled' : '';
    const pipelineMax = maxPipelineLength();
    const processingDisabled = state.continuousRecordEnabled ? ' is-disabled' : '';

    const body =
        '<div id="monitor-pause-banner" class="status-banner status-paused" role="status"' +
        (state.isPaused ? '' : ' hidden') +
        '>クリップボード監視は一時停止中です</div>' +
        '<div id="monitor-record-banner" class="status-banner status-continuous-record" role="status"' +
        (state.continuousRecordEnabled ? '' : ' hidden') +
        '>連続記録中はコピー時の自動加工を停止しています</div>' +
        card(
            '監視の状態',
            optionCheckbox(
                'monitor-paused',
                state.isPaused,
                'クリップボード監視を一時停止',
                '有効にすると加工と履歴の記録を止める'
            )
        ) +
        card(
            '監視方式',
            '' +
                '<div class="segmented" role="radiogroup" aria-label="監視方式">' +
                '<label><input type="radio" name="monitorMode" value="Polling"' +
                (state.monitorMode === 'Polling' ? ' checked' : '') +
                '> ポーリング</label>' +
                '<label><input type="radio" name="monitorMode" value="Event"' +
                (state.monitorMode === 'Event' ? ' checked' : '') +
                '> イベント</label>' +
                '</div>' +
                '<p class="hint">イベント方式は Windows のクリップボード変更通知を利用し、低 CPU で動作する</p>'
        ) +
        card(
            '監視周期',
            '' +
                '<div class="field' +
                intervalDisabled +
                '" id="interval-section">' +
                '<label for="interval-ms">ポーリング間隔</label>' +
                '<select id="interval-ms">' +
                INTERVAL_OPTIONS.map((option) => {
                    const selected = option.value === state.intervalMs ? ' selected' : '';
                    return (
                        '<option value="' +
                        option.value +
                        '"' +
                        selected +
                        '>' +
                        escapeHtml(option.label) +
                        '</option>'
                    );
                }).join('') +
                '</select>' +
                '<p class="hint">イベント監視時は使用されない</p></div>'
        ) +
        '<div id="monitor-processing-section"' +
        processingDisabled +
        '>' +
        card(
            'デフォルトモード',
            '' +
                '<div class="field">' +
                '<label for="default-mode">適用モード</label>' +
                '<select id="default-mode">' +
                buildModeOptions(state.mode) +
                '</select>' +
                '<p class="hint">パイプラインが空のときに監視で適用するモード</p></div>'
        ) +
        card(
            '加工パイプライン',
            '<div id="pipeline-list-container">' +
                buildPipelineInnerHtml() +
                '</div>' +
                '<div class="toolbar">' +
                '<select id="pipeline-add-mode">' +
                buildModeOptions('') +
                '</select>' +
                '<button type="button" class="ghost-btn" id="pipeline-add-btn">追加</button>' +
                '<button type="button" class="danger-btn" id="pipeline-clear-btn">クリア</button>' +
                '</div>' +
                '<p class="hint">最大 ' +
                pipelineMax +
                ' 段まで。空の場合はデフォルトモードのみ適用</p>' +
                '<p class="hint">Excel→画像 など画像出力モードは末尾に1つのみ配置され、途中に追加した場合は自動的に末尾へ移動する</p>'
        ) +
        '</div>';

    return panelWrap('monitor', body);
}

/** @returns {string} 通知タブのパネル HTML */
function renderNotificationsPanel() {
    const ns = state.notificationSettings;
    const body =
        card(
            'デスクトップ通知',
            '' +
                '<div class="option-list">' +
                optionCheckbox(
                    'notify-enabled',
                    ns.enabled,
                    'デスクトップ通知を有効にする',
                    'エラー通知は常に表示される'
                ) +
                '</div>'
        ) +
        card(
            '通知の種類',
            '' +
                '<div class="option-list notify-sub">' +
                optionCheckbox('notify-process', ns.notifyProcess, '加工完了時に通知') +
                optionCheckbox('notify-pause', ns.notifyPause, '一時停止切替時に通知') +
                optionCheckbox(
                    'notify-history-restore',
                    ns.notifyHistoryRestore,
                    '履歴復元時に通知'
                ) +
                optionCheckbox('notify-ocr', ns.notifyOcr, 'OCR 完了時に通知') +
                optionCheckbox(
                    'notify-registered-clip',
                    ns.notifyRegisteredClip,
                    '登録クリップ操作時に通知'
                ) +
                optionCheckbox('notify-favorites', ns.notifyFavorites, 'お気に入り操作時に通知') +
                optionCheckbox(
                    'notify-continuous-record',
                    ns.notifyContinuousRecord,
                    '連続記録操作時に通知'
                ) +
                '</div>'
        ) +
        card(
            '通知の内容',
            '' +
                '<div class="option-list notify-sub">' +
                optionCheckbox(
                    'notify-result',
                    ns.notifyResult,
                    '通知にクリップボードの内容を含める',
                    '加工完了通知などで内容を表示する'
                ) +
                '</div>' +
                '<p class="hint">個別設定はマスターがオンのときにのみ通知へ反映される</p>'
        );

    return panelWrap('notifications', body);
}

/** @returns {string} 履歴・連続記録タブのパネル HTML */
function renderHistoryPanel() {
    const historyBounds = historyLimitBounds();
    const recordBounds = recordLimitBounds();

    const historyCard = card(
        '履歴',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'history-enabled',
                state.historyEnabled,
                '履歴を有効にする',
                '加工したテキストを暗号化して保持する'
            ) +
            '</div>' +
            '<div class="field"><label for="history-limit">最大保持件数</label>' +
            '<input type="number" id="history-limit" min="' +
            historyBounds.min +
            '" max="' +
            historyBounds.max +
            '" value="' +
            state.historyLimit +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            historyBounds.min +
            '〜' +
            historyBounds.max +
            '</p></div>' +
            '<div class="card-actions"><button type="button" class="danger-btn" id="clear-history-btn">履歴をクリア</button></div>'
    );

    const recordCard = card(
        '連続記録',
        '' +
            '<div id="record-active-banner" class="status-banner status-continuous-record" role="status"' +
            (state.continuousRecordEnabled ? '' : ' hidden') +
            '>連続記録中はコピー時の自動加工を停止しています</div>' +
            '<div class="option-list">' +
            optionCheckbox(
                'record-enabled',
                state.continuousRecordEnabled,
                '連続記録を有効にする',
                '加工せず元テキストを順に記録する。有効中は自動加工が停止する'
            ) +
            '</div>' +
            '<div class="field"><label for="record-limit">最大保持件数</label>' +
            '<input type="number" id="record-limit" min="' +
            recordBounds.min +
            '" max="' +
            recordBounds.max +
            '" value="' +
            state.continuousRecordLimit +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            recordBounds.min +
            '〜' +
            recordBounds.max +
            '</p></div>' +
            '<div class="card-actions"><button type="button" class="danger-btn" id="clear-record-btn">連続記録をクリア</button></div>'
    );

    return panelWrap('history', '<div class="card-grid">' + historyCard + recordCard + '</div>');
}

/** @returns {string} 正規表現タブのパネル HTML */
function renderRegexPanel() {
    const regex = state.regex;
    const body =
        card(
            '正規表現の置換',
            '' +
                '<div class="field"><label for="regex-pattern">パターン</label>' +
                '<textarea id="regex-pattern">' +
                escapeHtml(regex.pattern) +
                '</textarea></div>' +
                '<div class="field"><label for="regex-replacement">置換文字列</label>' +
                '<textarea id="regex-replacement">' +
                escapeHtml(regex.replacement) +
                '</textarea></div>' +
                '<div class="option-list">' +
                optionCheckbox('regex-ci', regex.caseInsensitive, '大文字小文字を無視') +
                optionCheckbox('regex-ml', regex.multiline, '複数行モード') +
                '</div>' +
                '<p id="regex-status" class="regex-status" role="status" aria-live="polite"></p>'
        ) +
        card(
            'プレビュー',
            '' +
                '<div class="regex-preview">' +
                '<div class="regex-preview-row"><span class="regex-preview-label">サンプル</span>' +
                '<code class="regex-preview-sample">' +
                escapeHtml(state.regexPreviewSample) +
                '</code></div>' +
                '<div class="regex-preview-row"><span class="regex-preview-label">結果</span>' +
                '<code class="regex-preview-result" id="regex-preview-result">—</code></div>' +
                '</div>' +
                '<p class="hint">プレビューは ClipRefiner 実行時と同じ Rust 正規表現エンジンで表示</p>'
        );

    return panelWrap('regex', body);
}

/** @returns {string} お気に入りタブのパネル HTML */
function renderFavoritesPanel() {
    const favorites = state.favoriteModes || [];
    let listHtml = '';
    if (favorites.length === 0) {
        listHtml = '<div class="empty-hint">お気に入りは未登録です</div>';
    } else {
        listHtml = wrappedTable(
            '<table class="list-table"><thead><tr><th>モード</th><th>操作</th></tr></thead><tbody>' +
                SettingsPanelsLogic.buildFavoriteRowsHtml(favorites, modeLabel, escapeHtml) +
                '</tbody></table>'
        );
    }

    const availableModes = SettingsPanelsLogic.availableFavoriteModes(state.modes, favorites);
    const body = card(
        'お気に入り変換モード',
        listHtml +
            '<div class="favorite-add">' +
            '<select id="favorite-add-mode"><option value="">モードを選択...</option>' +
            availableModes
                .map((mode) => {
                    return (
                        '<option value="' +
                        escapeHtml(mode.id) +
                        '">' +
                        escapeHtml(mode.label) +
                        '</option>'
                    );
                })
                .join('') +
            '</select>' +
            '<button type="button" class="ghost-btn" id="favorite-add-btn">追加</button>' +
            '</div>' +
            '<p class="hint">最大 ' +
            maxFavoriteModes() +
            ' 件。お気に入り用ホットキーは「ショートカット」タブで編集</p>'
    );

    return panelWrap('favorites', body);
}

/** @returns {string} ショートカットタブのパネル HTML */
function renderShortcutsPanel() {
    const hotkeys = state.hotkeys || {};
    const favoriteModes = state.favoriteModes || [];
    const fixedRows = SettingsSharedLogic.HOTKEY_FIXED_LABELS.map(([key, label], index) => {
        const elementId = SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP[index][0];
        const value = hotkeys[key] || '';
        return (
            '<tr><td><label class="shortcut-label" for="' +
            elementId +
            '">' +
            escapeHtml(label) +
            '</label></td><td>' +
            '<input type="text" class="shortcut-input" id="' +
            elementId +
            '" data-hotkey-kind="fixed" data-hotkey-key="' +
            key +
            '" value="' +
            escapeHtml(value) +
            '" spellcheck="false" autocomplete="off" placeholder="Alt+Shift+S">' +
            '</td></tr>'
        );
    }).join('');

    let favoriteRows = '';
    if (favoriteModes.length === 0) {
        favoriteRows =
            '<tr><td colspan="2"><div class="empty-hint">お気に入り変換モードが未登録です</div></td></tr>';
    } else {
        favoriteRows = favoriteModes
            .map((modeId, index) => {
                const label = modeLabel(modeId);
                const display = SettingsPanelsLogic.favoriteSlotDisplay(
                    hotkeys.favoriteModeSlots,
                    index
                );
                const elementId = 'hotkey-favorite-slot-' + index;
                return (
                    '<tr><td><label class="shortcut-label" for="' +
                    elementId +
                    '">' +
                    escapeHtml(label) +
                    ' (お気に入り)</label>' +
                    '<p class="field-hint">空欄で無効</p></td><td>' +
                    '<input type="text" class="shortcut-input" id="' +
                    elementId +
                    '" data-hotkey-kind="favorite" data-hotkey-index="' +
                    index +
                    '" value="' +
                    escapeHtml(display.value) +
                    '" spellcheck="false" autocomplete="off" placeholder="' +
                    escapeHtml(display.placeholder) +
                    '">' +
                    '</td></tr>'
                );
            })
            .join('');
    }

    const body =
        '<div class="info-banner" role="note">' +
        '<div>形式は <code>Alt+Shift+S</code>。修飾キーは Alt / Shift / Ctrl / Win。' +
        'キーは A-Z / 0-9 / F1-F12。不正・重複は保存時に自動補正する。</div></div>' +
        card(
            '固定ホットキー',
            wrappedTable(
                '<table class="list-table shortcut-table"><thead><tr><th>動作</th><th>キー</th></tr></thead><tbody>' +
                    fixedRows +
                    '</tbody></table>'
            )
        ) +
        card(
            'お気に入りホットキー',
            wrappedTable(
                '<table class="list-table shortcut-table"><thead><tr><th>動作</th><th>キー</th></tr></thead><tbody>' +
                    favoriteRows +
                    '</tbody></table>'
            ) +
                '<div class="card-actions">' +
                '<button type="button" class="ghost-btn" id="shortcuts-open-config-btn">config.toml を開く</button>' +
                '</div>'
        );

    return panelWrap('shortcuts', body);
}

/** @returns {string} 一般タブのパネル HTML */
function renderGeneralPanel() {
    const body = card(
        '起動',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'launch-at-login',
                state.launchAtLogin,
                'ログイン時に起動',
                'Windows ログイン時に ClipRefiner を自動起動する'
            ) +
            '</div>' +
            '<p class="hint">実行ファイルを移動した場合は一度オフにしてから再度オンにする</p>'
    );

    return panelWrap('general', body);
}

/**
 * 登録クリップ 1 行分のテーブル行 HTML
 *
 * @param {{ id: string, label?: string, preview?: string, kind: string, thumbnail?: string }} clip
 * @returns {string}
 */
function renderClipRow(clip) {
    return SettingsPanelsLogic.renderClipRow(clip, escapeHtml);
}

/** @returns {string} 登録クリップタブのパネル HTML */
function renderClipsPanel() {
    const clips = state.clips || [];
    let listHtml = '';
    if (clips.length === 0) {
        listHtml = '<div class="empty-hint">登録クリップはありません</div>';
    } else {
        listHtml = wrappedTable(
            '<table class="list-table clip-table"><thead><tr><th>クリップ</th><th>操作</th></tr></thead><tbody>' +
                clips.map((clip) => renderClipRow(clip)).join('') +
                '</tbody></table>'
        );
    }

    const registerBtn = SettingsPanelsLogic.clipRegisterButtonState(clipRegisterPending);

    const body = card(
        '登録クリップ',
        '' +
            '<div class="toolbar"><button type="button" class="' +
            registerBtn.className +
            '" id="clip-register-btn"' +
            (registerBtn.disabled ? ' disabled' : '') +
            '>' +
            escapeHtml(registerBtn.label) +
            '</button></div>' +
            listHtml +
            '<p class="hint">最大 ' +
            maxRegisteredClips() +
            ' 件。画像はクリップボードに画像がある場合を優先</p>'
    );

    return panelWrap('clips', body);
}

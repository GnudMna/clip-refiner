// ======================================================================
// 設定画面 パネル HTML 生成
// ======================================================================

import { ctx } from './ctx.js';
import {
    TABS,
    advancedSection,
    buildModeOptions,
    card,
    escapeHtml,
    modeLabel,
    optionCheckbox,
    panelWrap,
    wrappedTable,
} from './html_builders.js';
import { buildPipelineInnerHtml } from './monitor_ui.js';
import { buildRegexEditorHtml } from './regex_ui.js';
import {
    historyLimitBounds,
    intervalMsBounds,
    maxFavoriteModes,
    maxPipelineLength,
    maxRegisteredClips,
    recordLimitBounds,
    selectorWebviewIdleMinutesBounds,
} from './shared.js';

import SettingsPanelsLogic from './panels_logic.js';
import SettingsSharedLogic from './shared_logic.js';

/** @type {Record<string, () => string>} */
export const PANEL_RENDERERS = {
    convert: renderConvertPanel,
    regex: renderRegexPanel,
    clipboard: renderClipboardPanel,
    notifications: renderNotificationsPanel,
    shortcuts: renderShortcutsPanel,
    general: renderGeneralPanel,
};

/** サイドバーのアクティブタブ表示を DOM に反映する */
function syncSidebarActiveTab() {
    const sidebar = document.getElementById('sidebar');
    if (!sidebar) {
        return;
    }
    sidebar.querySelectorAll('[role="tab"]').forEach((button) => {
        const isActive = button.dataset.tab === ctx.activeTab;
        const selection = SettingsPanelsLogic.tabSelectionState(isActive);
        button.classList.toggle('active', isActive);
        button.setAttribute('aria-selected', selection.ariaSelected);
        button.tabIndex = Number(selection.tabIndex);
    });
}

/** サイドバー tablist の内部 HTML を組み立てる */
function buildSidebarNavInnerHtml() {
    return SettingsPanelsLogic.buildSidebarNavInnerHtml(TABS, ctx.activeTab, escapeHtml);
}

/**
 * 指定タブへ切り替え、描画後にタブへフォーカスを移す
 *
 * @param {string} tabId
 * @param {{ focus?: boolean }} [options]
 */
function activateTab(tabId, options = {}) {
    const focus = options.focus !== false;
    if (tabId !== ctx.activeTab) {
        ctx.activeTab = tabId;
        ctx.render();
    } else {
        syncSidebarActiveTab();
    }
    if (focus) {
        document.getElementById('tab-' + tabId)?.focus();
    }
}

/**
 * サイドバーを初期化またはアクティブ状態を更新する
 *
 * タブ切替前に現在タブのスクロール位置を保存する
 */
export function ensureSidebar() {
    const content = document.getElementById('content');
    if (content) {
        ctx.tabScrollPositions[ctx.activeTab] = content.scrollTop;
    }

    const sidebar = document.getElementById('sidebar');
    if (!sidebar) {
        return;
    }

    if (!ctx.sidebarInitialized) {
        sidebar.innerHTML = buildSidebarNavInnerHtml();
        sidebar.addEventListener('click', (event) => {
            const button = event.target.closest('[role="tab"]');
            if (!button || !sidebar.contains(button)) {
                return;
            }
            activateTab(button.dataset.tab, { focus: true });
        });
        sidebar.addEventListener('keydown', (event) => {
            const button = event.target.closest('[role="tab"]');
            if (!button || !sidebar.contains(button)) {
                return;
            }
            const tabs = Array.from(sidebar.querySelectorAll('[role="tab"]'));
            const currentIndex = tabs.indexOf(button);
            const nextIndex = SettingsPanelsLogic.nextTabIndexForKey(
                currentIndex,
                tabs.length,
                event.key
            );
            if (nextIndex === null) {
                return;
            }
            event.preventDefault();
            activateTab(tabs[nextIndex].dataset.tab, { focus: true });
        });
        ctx.sidebarInitialized = true;
        return;
    }

    syncSidebarActiveTab();
}

/** @returns {string} 一般タブの監視の状態カード HTML */
function renderMonitorStateCards() {
    return (
        '<div id="monitor-pause-banner" class="status-banner status-paused" role="status"' +
        (ctx.state.isPaused ? '' : ' hidden') +
        '>クリップボード監視は一時停止中です</div>' +
        card(
            '監視の状態',
            optionCheckbox(
                'monitor-paused',
                ctx.state.isPaused,
                'クリップボード監視を一時停止',
                '有効にすると加工と履歴の記録を止める'
            )
        )
    );
}

/** @returns {string} 一般タブの詳細設定に置く監視方式・監視周期カード HTML */
function renderMonitorAdvancedCards() {
    const intervalDisabled = ctx.state.monitorMode === 'Event' ? ' disabled' : '';
    const intervalBounds = intervalMsBounds();

    return (
        card(
            '監視方式',
            '' +
                '<div class="segmented" role="radiogroup" aria-label="監視方式">' +
                '<label><input type="radio" name="monitorMode" value="Polling"' +
                (ctx.state.monitorMode === 'Polling' ? ' checked' : '') +
                '> ポーリング</label>' +
                '<label><input type="radio" name="monitorMode" value="Event"' +
                (ctx.state.monitorMode === 'Event' ? ' checked' : '') +
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
                '<label for="interval-ms">ポーリング間隔 (ミリ秒)</label>' +
                '<input type="number" id="interval-ms" min="' +
                intervalBounds.min +
                '" max="' +
                intervalBounds.max +
                '" step="1" value="' +
                ctx.state.intervalMs +
                '">' +
                '<p class="field-hint">有効範囲: ' +
                intervalBounds.min +
                '〜' +
                intervalBounds.max +
                ' ms。イベント監視時は使用されない</p></div>'
        )
    );
}

/** @returns {string} 変換タブのパネル HTML */
function renderConvertPanel() {
    const pipelineMax = maxPipelineLength();
    const processingDisabled = ctx.state.continuousRecordEnabled ? ' is-disabled' : '';

    const body =
        '<div id="monitor-record-banner" class="status-banner status-continuous-record" role="status"' +
        (ctx.state.continuousRecordEnabled ? '' : ' hidden') +
        '>連続記録中はコピー時の自動加工を停止しています</div>' +
        '<div id="monitor-processing-section"' +
        processingDisabled +
        '>' +
        card(
            '自動変換',
            optionCheckbox(
                'auto-refine-enabled',
                ctx.state.autoRefineEnabled,
                'コピー時に自動変換する',
                '無効のときはクイックセレクターやお気に入りホットキーで手動変換だけを行う'
            )
        ) +
        card(
            'デフォルトモード',
            '' +
                '<div class="field">' +
                '<label for="default-mode">適用モード</label>' +
                '<select id="default-mode">' +
                buildModeOptions(ctx.state.mode) +
                '</select>' +
                '<p class="hint">パイプラインが空のときに自動変換で適用するモード</p></div>'
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
        '</div>' +
        renderFavoritesCard();

    return panelWrap('convert', body);
}

/** @returns {string} 通知タブのパネル HTML */
function renderNotificationsPanel() {
    const ns = ctx.state.notificationSettings;
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
                    '加工完了通知などに内容を表示する。機密らしい内容は表示のみマスクする'
                ) +
                '</div>' +
                '<p class="hint">個別設定はマスターがオンのときにのみ通知へ反映される</p>' +
                '<p class="hint">マスキングは通知・メニュー・プレビューの表示置換のみ。クリップボード本体は書き換えない</p>'
        );

    return panelWrap('notifications', body);
}

/** @returns {string} クリップボードタブのパネル HTML */
function renderClipboardPanel() {
    const historyBounds = historyLimitBounds();
    const recordBounds = recordLimitBounds();

    const historyCard = card(
        '履歴',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'history-enabled',
                ctx.state.historyEnabled,
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
            ctx.state.historyLimit +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            historyBounds.min +
            '〜' +
            historyBounds.max +
            '</p></div>' +
            '<p class="hint">トレイ履歴のラベルでは機密らしい内容を表示のみマスクする。復元時はクリップボードへ元の本文を載せる</p>' +
            '<p class="hint">保護境界は現在の Windows ユーザー単位。同一ユーザーの他プロセスからは設定・IPC・鍵に到達しうる</p>' +
            '<div class="card-actions"><button type="button" class="danger-btn" id="clear-history-btn">履歴をクリア</button></div>'
    );

    const recordCard = card(
        '連続記録',
        '' +
            '<div id="record-active-banner" class="status-banner status-continuous-record" role="status"' +
            (ctx.state.continuousRecordEnabled ? '' : ' hidden') +
            '>連続記録中はコピー時の自動加工を停止しています</div>' +
            '<div class="option-list">' +
            optionCheckbox(
                'record-enabled',
                ctx.state.continuousRecordEnabled,
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
            ctx.state.continuousRecordLimit +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            recordBounds.min +
            '〜' +
            recordBounds.max +
            '</p></div>' +
            '<div class="card-actions"><button type="button" class="danger-btn" id="clear-record-btn">連続記録をクリア</button></div>'
    );

    return panelWrap(
        'clipboard',
        '<div class="card-grid">' + historyCard + recordCard + '</div>' + renderClipsCard()
    );
}

/** @returns {string} 正規表現タブのパネル HTML */
function renderRegexPanel() {
    const body =
        card('正規表現プリセット', buildRegexEditorHtml()) +
        card(
            'プレビュー',
            '' +
                '<div class="field"><label for="regex-preview-sample">サンプル</label>' +
                '<textarea id="regex-preview-sample" rows="3">' +
                escapeHtml(ctx.state.regexPreviewSample) +
                '</textarea></div>' +
                '<div class="regex-preview">' +
                '<div class="regex-preview-row"><span class="regex-preview-label">結果</span>' +
                '<code class="regex-preview-result" id="regex-preview-result">—</code></div>' +
                '</div>' +
                '<p class="hint">選択中プリセットのステップを上から順に regex-replace で適用。本番と同じ Rust 正規表現エンジン</p>'
        );

    return panelWrap('regex', body);
}

/** @returns {string} 変換タブのお気に入り変換モードカード HTML */
function renderFavoritesCard() {
    const favorites = ctx.state.favoriteModes || [];
    const listHtml =
        favorites.length === 0
            ? '<div class="empty-hint">お気に入りは未登録です</div>'
            : wrappedTable(
                  '<table class="list-table"><thead><tr><th>モード</th><th>操作</th></tr></thead><tbody>' +
                      SettingsPanelsLogic.buildFavoriteRowsHtml(favorites, modeLabel, escapeHtml) +
                      '</tbody></table>'
              );

    const availableModes = SettingsPanelsLogic.availableFavoriteModes(ctx.state.modes, favorites);
    return card(
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
            ' 件。お気に入りは現在のクリップボードへ一回だけ適用する。ホットキーは「ショートカット」タブで編集</p>'
    );
}

/** @returns {string} ショートカットタブのパネル HTML */
function renderShortcutsPanel() {
    const hotkeys = ctx.state.hotkeys || {};
    const favoriteModes = ctx.state.favoriteModes || [];
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

    const favoriteRows =
        favoriteModes.length === 0
            ? '<tr><td colspan="2"><div class="empty-hint">お気に入り変換モードが未登録です</div></td></tr>'
            : favoriteModes
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

    const body =
        '<div class="info-banner" role="note">' +
        '<div>形式は <code>Alt+Shift+S</code>。修飾キーは Alt / Shift / Ctrl / Win。' +
        'キーは A-Z / 0-9 / F1-F24 / Space / Enter / Tab / Esc / 矢印キー / Home / End / PageUp / PageDown / Insert / Delete / Numpad0-9 / - , . / など。' +
        '不正・重複は保存時に自動補正する。</div></div>' +
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

/**
 * 更新ステータス表示 HTML を組み立てる
 *
 * @returns {string}
 */
function renderUpdateStatusHtml() {
    return SettingsPanelsLogic.renderUpdateStatusHtml(ctx.updateStatus, ctx.updateBusy, escapeHtml);
}

/** @returns {string} 一般タブのパネル HTML */
function renderGeneralPanel() {
    const version = String(ctx.state?.appVersion ?? '');
    const portable = Boolean(ctx.state?.portableMode);
    const edition = portable ? 'ポータブル' : 'インストール版 (MSI)';
    const checkLabel = ctx.updateBusy ? '確認中...' : '更新を確認';
    const retention = ctx.state.selectorWebviewRetention || 'Keep';
    const idleDisabled = retention === 'DisposeAfterIdle' ? '' : ' disabled';
    const idleBounds = selectorWebviewIdleMinutesBounds();
    const idleMinutes = ctx.state.selectorWebviewIdleMinutes ?? 5;

    const launchCard = card(
        '起動',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'launch-at-login',
                ctx.state.launchAtLogin,
                'ログイン時に起動',
                'Windows ログイン時に ClipRefiner を自動起動する'
            ) +
            '</div>' +
            '<p class="hint">実行ファイルを移動した場合は一度オフにしてから再度オンにする</p>'
    );

    const selectorCard = card(
        'セレクター WebView',
        '' +
            '<div class="segmented" role="radiogroup" aria-label="セレクター WebView の保持">' +
            '<label><input type="radio" name="selectorWebviewRetention" value="Keep"' +
            (retention === 'Keep' ? ' checked' : '') +
            '> 保持する</label>' +
            '<label><input type="radio" name="selectorWebviewRetention" value="DisposeOnHide"' +
            (retention === 'DisposeOnHide' ? ' checked' : '') +
            '> 非表示時に破棄</label>' +
            '<label><input type="radio" name="selectorWebviewRetention" value="DisposeAfterIdle"' +
            (retention === 'DisposeAfterIdle' ? ' checked' : '') +
            '> アイドル後に破棄</label>' +
            '</div>' +
            '<p class="hint">クイック / 登録クリップセレクターの WebView。保持すると再表示が速く、破棄するとアイドル時のメモリを抑えられる</p>' +
            '<div class="field' +
            idleDisabled +
            '" id="selector-webview-idle-section">' +
            '<label for="selector-webview-idle-minutes">アイドル分数</label>' +
            '<input type="number" id="selector-webview-idle-minutes" min="' +
            idleBounds.min +
            '" max="' +
            idleBounds.max +
            '" step="1" value="' +
            idleMinutes +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            idleBounds.min +
            '〜' +
            idleBounds.max +
            ' 分。「アイドル後に破棄」選択時のみ使用</p></div>'
    );

    const versionCard = card(
        'バージョン',
        '' +
            '<p class="hint">現在のバージョン: v' +
            escapeHtml(version) +
            ' (' +
            escapeHtml(edition) +
            ')</p>' +
            '<div class="update-actions">' +
            '<button type="button" id="check-update-btn" class="ghost-btn"' +
            (ctx.updateBusy ? ' disabled' : '') +
            '>' +
            escapeHtml(checkLabel) +
            '</button>' +
            '</div>' +
            renderUpdateStatusHtml()
    );

    return panelWrap(
        'general',
        renderMonitorStateCards() +
            launchCard +
            versionCard +
            advancedSection(
                '詳細設定',
                renderMonitorAdvancedCards() + selectorCard,
                ctx.advancedSectionOpen
            )
    );
}

/**
 * 登録クリップ 1 行分のテーブル行 HTML
 *
 * @param {{ id: string, label?: string, preview?: string, kind: string, thumbnail?: string }} clip
 * @param {number} index
 * @param {number} count
 * @returns {string}
 */
function renderClipRow(clip, index, count) {
    return SettingsPanelsLogic.renderClipRow(clip, escapeHtml, index, count);
}

/** @returns {string} クリップボードタブの登録クリップカード HTML */
function renderClipsCard() {
    const clips = ctx.state.clips || [];
    const listHtml =
        clips.length === 0
            ? '<div class="empty-hint">登録クリップはありません</div>'
            : wrappedTable(
                  '<table class="list-table clip-table"><thead><tr><th>クリップ</th><th>操作</th></tr></thead><tbody>' +
                      clips
                          .map((clip, index) => renderClipRow(clip, index, clips.length))
                          .join('') +
                      '</tbody></table>'
              );

    const registerBtn = SettingsPanelsLogic.clipRegisterButtonState(ctx.clipRegisterPending);

    return card(
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
            ' 件。画像はクリップボードに画像がある場合を優先</p>' +
            '<p class="hint">プレビューのマスキングは表示のみ。コピー時はクリップボードへ元の本文を載せる</p>' +
            '<p class="hint">鍵は現在ユーザーの DPAPI で保護する。同一ユーザーの他プロセスからは設定・鍵に到達しうる</p>'
    );
}

// ======================================================================
// 設定画面 パネル HTML 生成
// ======================================================================

import { ctx, loadedState } from './ctx.ts';
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
} from './html_builders.ts';
import { buildPipelineInnerHtml } from './monitor_ui.ts';
import { buildRegexEditorHtml } from './regex/regex_ui.ts';
import {
    historyLimitBounds,
    intervalMsBounds,
    maxFavoriteModes,
    maxPipelineLength,
    maxRegisteredClips,
    recordLimitBounds,
    selectorWebviewIdleMinutesBounds,
} from './limits.ts';

import SettingsPanelsLogic from './panels_logic.ts';
import SettingsSharedLogic from './shared_logic.ts';

import type { ClipSelectorItem } from '../selector/clip_selector_logic.ts';
import type { HotkeySettings } from '../ipc.ts';

export const PANEL_RENDERERS: Record<string, () => string> = {
    monitor: renderMonitorPanel,
    convert: renderConvertPanel,
    regex: renderRegexPanel,
    clipboard: renderClipboardPanel,
    notifications: renderNotificationsPanel,
    shortcuts: renderShortcutsPanel,
    general: renderGeneralPanel,
    about: renderAboutPanel,
};

/** サイドバーのアクティブタブ表示を DOM に反映する */
function syncSidebarActiveTab(): void {
    const sidebar = document.getElementById('sidebar');
    if (!sidebar) {
        return;
    }
    sidebar.querySelectorAll<HTMLElement>('[role="tab"]').forEach((button) => {
        const isActive = button.dataset.tab === ctx.activeTab;
        const selection = SettingsPanelsLogic.tabSelectionState(isActive);
        button.classList.toggle('active', isActive);
        button.setAttribute('aria-selected', selection.ariaSelected);
        button.tabIndex = Number(selection.tabIndex);
    });
}

/** サイドバー tablist の内部 HTML を組み立てる */
function buildSidebarNavInnerHtml(): string {
    return SettingsPanelsLogic.buildSidebarNavInnerHtml(TABS, ctx.activeTab, escapeHtml);
}

/**
 * 指定タブへ切り替え、描画後にタブへフォーカスを移す
 */
function activateTab(tabId: string, options: { focus?: boolean } = {}): void {
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
export function ensureSidebar(): void {
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
            const button: HTMLElement | null = (event.target as Element).closest('[role="tab"]');
            const tab = button?.dataset.tab;
            if (!button || !tab || !sidebar.contains(button)) {
                return;
            }
            activateTab(tab, { focus: true });
        });
        sidebar.addEventListener('keydown', (event) => {
            const button: HTMLElement | null = (event.target as Element).closest('[role="tab"]');
            if (!button || !sidebar.contains(button)) {
                return;
            }
            const tabs: HTMLElement[] = Array.from(sidebar.querySelectorAll('[role="tab"]'));
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
            const tab = tabs[nextIndex]?.dataset.tab;
            if (tab) {
                activateTab(tab, { focus: true });
            }
        });
        ctx.sidebarInitialized = true;
        return;
    }

    syncSidebarActiveTab();
}

/** 監視タブのパネル HTML (監視方式・監視周期カード) */
function renderMonitorPanel(): string {
    const intervalDisabled = loadedState().monitorMode === 'Event' ? ' disabled' : '';
    const intervalBounds = intervalMsBounds();

    const body =
        card(
            '監視方式',
            '' +
                '<div class="segmented" role="radiogroup" aria-label="監視方式">' +
                '<label><input type="radio" name="monitorMode" value="Polling"' +
                (loadedState().monitorMode === 'Polling' ? ' checked' : '') +
                '> ポーリング</label>' +
                '<label><input type="radio" name="monitorMode" value="Event"' +
                (loadedState().monitorMode === 'Event' ? ' checked' : '') +
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
                loadedState().intervalMs +
                '">' +
                '<p class="field-hint">有効範囲: ' +
                intervalBounds.min +
                '〜' +
                intervalBounds.max +
                ' ms。イベント監視時は使用されない</p></div>'
        );

    return panelWrap('monitor', body);
}

/** 変換タブのパネル HTML */
function renderConvertPanel(): string {
    const pipelineMax = maxPipelineLength();
    const processingDisabled = loadedState().continuousRecordEnabled ? ' is-disabled' : '';

    const body =
        '<div id="monitor-record-banner" class="status-banner status-continuous-record" role="status"' +
        (loadedState().continuousRecordEnabled ? '' : ' hidden') +
        '>連続記録モード中はコピー時の自動加工を停止しています。自動変換を有効にすると連続記録モードは無効になります</div>' +
        card(
            '自動変換',
            optionCheckbox(
                'auto-refine-enabled',
                loadedState().autoRefineEnabled,
                'コピー時に自動変換する',
                '無効のときはクイックセレクターやお気に入りホットキーで手動変換だけを行う。連続記録モードとは同時に有効にならない'
            )
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
                buildModeOptions(loadedState().mode) +
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

/** 通知タブのパネル HTML */
function renderNotificationsPanel(): string {
    const ns = loadedState().notificationSettings;
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
                optionCheckbox(
                    'notify-auto-refine',
                    ns.notifyAutoRefine,
                    '自動変換の切替時に通知'
                ) +
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
                    '連続記録モードの操作時に通知'
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

/** クリップボードタブのパネル HTML */
function renderClipboardPanel(): string {
    const historyBounds = historyLimitBounds();
    const recordBounds = recordLimitBounds();

    const historyCard = card(
        '履歴',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'history-enabled',
                loadedState().historyEnabled,
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
            loadedState().historyLimit +
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
        '連続記録モード',
        '' +
            '<div id="record-active-banner" class="status-banner status-continuous-record" role="status"' +
            (loadedState().continuousRecordEnabled ? '' : ' hidden') +
            '>連続記録モード中はコピー時の自動加工を停止しています</div>' +
            '<div class="option-list">' +
            optionCheckbox(
                'record-enabled',
                loadedState().continuousRecordEnabled,
                '連続記録モードを有効にする',
                '加工せず元テキストを順に記録する。有効にすると自動変換は無効になる'
            ) +
            '</div>' +
            '<div class="field"><label for="record-limit">最大保持件数</label>' +
            '<input type="number" id="record-limit" min="' +
            recordBounds.min +
            '" max="' +
            recordBounds.max +
            '" value="' +
            loadedState().continuousRecordLimit +
            '">' +
            '<p class="field-hint">有効範囲: ' +
            recordBounds.min +
            '〜' +
            recordBounds.max +
            '</p></div>' +
            '<div class="card-actions"><button type="button" class="danger-btn" id="clear-record-btn">記録をクリア</button></div>'
    );

    const exclusionCard = card(
        '除外形式',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'keep-exclusion',
                loadedState().keepClipboardExclusion,
                '除外形式を付けてコピーする',
                'パスワードマネージャーのコピーを手動加工したとき、結果にも除外形式を付ける。登録クリップのコピーにも付ける'
            ) +
            '</div>' +
            '<p class="hint">Win+V 履歴・クラウド同期・本アプリの監視から除外する。貼り付けは通常どおりできる</p>'
    );

    return panelWrap(
        'clipboard',
        '<div class="card-grid">' +
            historyCard +
            recordCard +
            exclusionCard +
            '</div>' +
            renderClipsCard()
    );
}

/** 正規表現タブのパネル HTML */
function renderRegexPanel(): string {
    const body =
        card('正規表現プリセット', buildRegexEditorHtml()) +
        card(
            'プレビュー',
            '' +
                '<div class="field"><label for="regex-preview-sample">サンプル</label>' +
                '<textarea id="regex-preview-sample" rows="3">' +
                escapeHtml(loadedState().regexPreviewSample) +
                '</textarea></div>' +
                '<div class="regex-preview">' +
                '<div class="regex-preview-row"><span class="regex-preview-label">結果</span>' +
                '<code class="regex-preview-result" id="regex-preview-result">—</code></div>' +
                '</div>' +
                '<p class="hint">選択中プリセットのステップを上から順に regex-replace で適用。本番と同じ Rust 正規表現エンジン</p>'
        );

    return panelWrap('regex', body);
}

/** 変換タブのお気に入り変換モードカード HTML */
function renderFavoritesCard(): string {
    const favorites = loadedState().favoriteModes || [];
    const listHtml =
        favorites.length === 0
            ? '<div class="empty-hint">お気に入りは未登録です</div>'
            : wrappedTable(
                  '<table class="list-table"><thead><tr><th>モード</th><th>操作</th></tr></thead><tbody>' +
                      SettingsPanelsLogic.buildFavoriteRowsHtml(favorites, modeLabel, escapeHtml) +
                      '</tbody></table>'
              );

    const availableModes = SettingsPanelsLogic.availableFavoriteModes(
        loadedState().modes,
        favorites
    );
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

/** ショートカットタブのパネル HTML */
function renderShortcutsPanel(): string {
    const hotkeys: Partial<HotkeySettings> = loadedState().hotkeys || {};
    const favoriteModes = loadedState().favoriteModes || [];
    const fixedRows = SettingsSharedLogic.HOTKEY_FIXED_LABELS.map(([key, label], index) => {
        const elementId = SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP[index]?.[0] ?? '';
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
        "キーは A-Z / 0-9 / F1-F24 / Space / Enter / Tab / Esc / 矢印キー / Home / End / PageUp / PageDown / Insert / Delete / Numpad0-9 / 記号キー (- , . / ; = ` [ \\ ] ') など。" +
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

/** 一般タブのパネル HTML */
function renderGeneralPanel(): string {
    const version = String(ctx.state?.appVersion ?? '');
    const portable = Boolean(ctx.state?.portableMode);
    const edition = portable ? 'ポータブル' : 'インストール版 (MSI)';
    const retention = loadedState().selectorWebviewRetention || 'Keep';
    const idleDisabled = retention === 'DisposeAfterIdle' ? '' : ' disabled';
    const idleBounds = selectorWebviewIdleMinutesBounds();
    const idleMinutes = loadedState().selectorWebviewIdleMinutes ?? 5;

    const launchCard = card(
        '起動',
        '' +
            '<div class="option-list">' +
            optionCheckbox(
                'launch-at-login',
                loadedState().launchAtLogin,
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
            '<button type="button" id="check-update-btn" class="ghost-btn">更新を確認</button>' +
            '</div>' +
            '<p class="hint">確認の結果と更新の進行は、別の画面に表示する</p>'
    );

    return panelWrap(
        'general',
        launchCard +
            versionCard +
            advancedSection('詳細設定', selectorCard, ctx.advancedSectionOpen)
    );
}

/** 情報タブのパネル HTML */
function renderAboutPanel(): string {
    return panelWrap(
        'about',
        card(
            '利用者向けドキュメント',
            '' +
                '<p class="hint">使い方・設定の詳細をブラウザで開く</p>' +
                '<div class="card-actions"><button type="button" class="ghost-btn" id="open-user-docs-btn">ドキュメントを開く</button></div>'
        ) +
            // 表記は LICENSE の冒頭 2 行 (空行を除く) と同じ。ずれは tests/flow/license_ui.rs が検出する
            card(
                'ライセンス',
                '' +
                    '<p class="hint">Copyright (c) 2026 Yuto Hirata</p>' +
                    '<p class="hint">MIT License</p>' +
                    '<div class="card-actions"><button type="button" class="ghost-btn" id="open-license-btn">LICENSE を開く</button></div>'
            ) +
            card(
                '第三者ライセンス',
                '' +
                    '<p class="hint">利用者向けドキュメントに含まれる mdBook のテーマ・highlight.js・clipboard.js・Font Awesome を含む、第三者ソフトウェアのライセンス表示</p>' +
                    '<div class="card-actions"><button type="button" class="ghost-btn" id="open-third-party-notices-btn">THIRD_PARTY_NOTICES を開く</button></div>'
            )
    );
}

/**
 * 登録クリップ 1 行分のテーブル行 HTML
 */
function renderClipRow(clip: ClipSelectorItem, index: number, count: number): string {
    return SettingsPanelsLogic.renderClipRow(clip, escapeHtml, index, count);
}

/** クリップボードタブの登録クリップカード HTML */
function renderClipsCard(): string {
    const clips = loadedState().clips || [];
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

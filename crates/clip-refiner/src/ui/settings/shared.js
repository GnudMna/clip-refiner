// ======================================================================
// 設定画面 UI 共有状態・フォーム補助
// ======================================================================

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

/**
 * ポーリング間隔 (ミリ秒) の min/max
 *
 * @returns {{ min: number, max: number }}
 */
function intervalMsBounds() {
    const l = limits();
    return {
        min: l.intervalMinMs ?? 100,
        max: l.intervalMaxMs ?? 60_000,
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
 * 数値入力を min/max の範囲にクランプする
 *
 * @param {string|number} raw
 * @param {number} min
 * @param {number} max
 * @returns {{ value: number, clamped: boolean }}
 */
function clampLimitValue(raw, min, max) {
    return SettingsSharedLogic.clampLimitValue(raw, min, max);
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
        showFieldFeedback(input, bounds.min + '〜' + bounds.max + ' の範囲に調整しました');
    } else {
        clearFieldFeedback(input);
    }
    state[stateKey] = result.value;
    scheduleApply({ [patchKey]: result.value });
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
    updateClipRegisterButton();
    if (activeTab === 'shortcuts') {
        syncHotkeyInputsFromState();
    }
}

/** ショートカット入力欄を `state.hotkeys` へ同期する (フォーカス中はスキップ) */
function syncHotkeyInputsFromState() {
    const hotkeys = state.hotkeys || {};
    const active = document.activeElement;

    for (const [elementId, key] of SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP) {
        const element = document.getElementById(elementId);
        if (!element || element === active) {
            continue;
        }
        const next = hotkeys[key] || '';
        if (element.value !== next) {
            element.value = next;
        }
        element.classList.remove('is-invalid');
    }

    const favoriteModes = state.favoriteModes || [];
    const slots = Array.isArray(hotkeys.favoriteModeSlots) ? hotkeys.favoriteModeSlots : [];
    for (let index = 0; index < favoriteModes.length; index += 1) {
        const element = document.getElementById('hotkey-favorite-slot-' + index);
        if (!element || element === active) {
            continue;
        }
        const placeholder = index < 9 ? 'Alt+Shift+' + (index + 1) : 'Alt+Shift+F' + (index - 8);
        const custom = index < slots.length ? slots[index] : undefined;
        const next = custom === undefined ? placeholder : custom;
        if (element.value !== next) {
            element.value = next;
        }
        element.classList.remove('is-invalid');
    }
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

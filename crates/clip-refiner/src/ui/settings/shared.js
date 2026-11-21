// ======================================================================
// 設定画面 UI 共有状態・フォーム補助
// ======================================================================

import { ctx } from './ctx.js';
import { scheduleApply } from './ipc_apply.js';
import { syncContinuousRecordUi, syncMonitorIntervalSection } from './monitor_ui.js';

import SettingsSharedLogic from './shared_logic.js';

/** セレクター WebView 保持方針に応じてアイドル分数欄の有効状態を同期する */
export function syncSelectorWebviewIdleSection() {
    const section = document.getElementById('selector-webview-idle-section');
    if (section) {
        section.classList.toggle(
            'disabled',
            ctx.state.selectorWebviewRetention !== 'DisposeAfterIdle'
        );
    }
}

/**
 * サーバーから渡された制限値オブジェクトを返す
 *
 * @returns {Record<string, number>}
 */
export function limits() {
    return ctx.state?.limits ?? {};
}

/**
 * 履歴保持件数の min/max
 *
 * @returns {{ min: number, max: number }}
 */
export function historyLimitBounds() {
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
export function recordLimitBounds() {
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
export function intervalMsBounds() {
    const l = limits();
    return {
        min: l.intervalMinMs ?? 100,
        max: l.intervalMaxMs ?? 60_000,
    };
}

/**
 * セレクター WebView アイドル破棄分数の min/max
 *
 * @returns {{ min: number, max: number }}
 */
export function selectorWebviewIdleMinutesBounds() {
    const l = limits();
    return {
        min: l.selectorWebviewIdleMinMinutes ?? 1,
        max: l.selectorWebviewIdleMaxMinutes ?? 1440,
    };
}

/** @returns {number} 加工パイプラインの最大段数 */
export function maxPipelineLength() {
    return limits().maxPipelineLength ?? 10;
}

/** @returns {number} お気に入りモードの最大件数 */
export function maxFavoriteModes() {
    return limits().maxFavoriteModes ?? 20;
}

/** @returns {number} 登録クリップの最大件数 */
export function maxRegisteredClips() {
    return limits().maxRegisteredClips ?? 100;
}

/** @returns {number} 登録クリップのラベルの最大文字数 */
export function maxRegisteredClipLabelChars() {
    return limits().maxRegisteredClipLabelChars ?? 64;
}

/** @returns {number} 正規表現パターンの最大バイト数 */
export function maxRegexPatternBytes() {
    return limits().maxRegexPatternBytes ?? 8 * 1024;
}

/** @returns {number} 正規表現プリセットの最大件数 */
export function maxRegexPresets() {
    return limits().maxRegexPresets ?? 20;
}

/** @returns {number} 正規表現プリセットあたりの最大ステップ数 */
export function maxRegexStepsPerPreset() {
    return limits().maxRegexStepsPerPreset ?? 10;
}

/**
 * コンテンツ領域でフォーム要素が編集中かどうか
 *
 * @returns {boolean}
 */
export function isContentEditing() {
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
export function clampLimitValue(raw, min, max) {
    return SettingsSharedLogic.clampLimitValue(raw, min, max);
}

/**
 * フィールドに調整フィードバックを表示する
 *
 * @param {HTMLElement} input
 * @param {string} message
 */
export function showFieldFeedback(input, message) {
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
export function clearFieldFeedback(input) {
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
export function commitLimitInput(input, patchKey, stateKey, bounds) {
    const result = clampLimitValue(input.value, bounds.min, bounds.max);
    input.value = String(result.value);
    if (result.clamped) {
        showFieldFeedback(input, bounds.min + '〜' + bounds.max + ' の範囲に調整しました');
    } else {
        clearFieldFeedback(input);
    }
    ctx.state[stateKey] = result.value;
    scheduleApply({ [patchKey]: result.value });
}

/**
 * 指定タブのパネルをキャッシュから破棄する
 *
 * @param {string} tabId
 */
export function invalidatePanel(tabId) {
    ctx.renderedPanels.delete(tabId);
    ctx.boundPanels.delete(tabId);
    document.getElementById('panel-' + tabId)?.remove();
}

/** 全タブのパネルキャッシュを破棄しコンテンツを空にする */
export function invalidateAllPanels() {
    ctx.renderedPanels.clear();
    ctx.boundPanels.clear();
    const content = document.getElementById('content');
    if (content) {
        content.innerHTML = '';
    }
}

/**
 * コンテンツのスクロール位置をアニメーションなしで設定する
 *
 * `#content` は `scroll-behavior: smooth` のため、`scrollTop` 代入では復元が滑って見える
 *
 * @param {HTMLElement} content
 * @param {number} top
 */
function setContentScrollTop(content, top) {
    content.scrollTo({ top, behavior: 'instant' });
}

/** アクティブタブのパネルを表示しスクロール位置を復元する */
export function showActivePanel() {
    const content = document.getElementById('content');
    if (!content) {
        return;
    }
    content.querySelectorAll('.panel').forEach((panel) => {
        const isActive = panel.id === 'panel-' + ctx.activeTab;
        panel.classList.toggle('active', isActive);
        panel.hidden = !isActive;
    });
    setContentScrollTop(content, ctx.tabScrollPositions[ctx.activeTab] || 0);
}

/**
 * パネルの破棄・再描画を挟んでもスクロール位置を保つ
 *
 * パネルを破棄するとコンテンツの高さが縮み、ブラウザが `scrollTop` を 0 へ丸めるため、
 * 実行前の位置を控えて実行後に戻す
 *
 * @param {() => void} action - パネルを再描画する処理
 */
export function withPreservedScroll(action) {
    const content = document.getElementById('content');
    const top = content ? content.scrollTop : 0;
    action();
    if (content) {
        ctx.tabScrollPositions[ctx.activeTab] = top;
        setContentScrollTop(content, top);
    }
}

/** サーバー主導のフィールド (自動変換・連続記録など) を DOM に反映する */
export function syncServerFields() {
    const launch = document.getElementById('launch-at-login');
    if (launch) {
        launch.checked = ctx.state.launchAtLogin;
    }
    const autoRefine = document.getElementById('auto-refine-enabled');
    if (autoRefine) {
        autoRefine.checked = ctx.state.autoRefineEnabled;
    }
    syncContinuousRecordUi();
    syncMonitorIntervalSection();
    syncSelectorWebviewIdleSection();
    updateClipRegisterButton();
    if (ctx.activeTab === 'shortcuts') {
        syncHotkeyInputsFromState();
    }
}

/** ショートカット入力欄を `state.hotkeys` へ同期する (フォーカス中はスキップ) */
export function syncHotkeyInputsFromState() {
    const hotkeys = ctx.state.hotkeys || {};
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

    const favoriteModes = ctx.state.favoriteModes || [];
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
export function setClipRegisterPending(pending) {
    ctx.clipRegisterPending = pending;
    updateClipRegisterButton();
}

/** クリップ登録ボタンの表示を `clipRegisterPending` に合わせて更新する */
export function updateClipRegisterButton() {
    const btn = document.getElementById('clip-register-btn');
    if (!btn) {
        return;
    }
    btn.disabled = ctx.clipRegisterPending;
    btn.textContent = ctx.clipRegisterPending ? '登録中...' : 'クリップボードから登録';
    btn.classList.toggle('is-loading', ctx.clipRegisterPending);
}

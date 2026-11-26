// ======================================================================
// 設定画面 UI 共有状態・フォーム補助
// ======================================================================

import { buttonById, inputById } from '../shared/dom.ts';
import { ctx, loadedState } from './ctx.ts';
import { scheduleApply } from './ipc_apply.ts';
import { syncContinuousRecordUi, syncMonitorIntervalSection } from './monitor_ui.ts';

import SettingsSharedLogic from './shared_logic.ts';

import type { HotkeySettings } from '../ipc.ts';

/** セレクター WebView 保持方針に応じてアイドル分数欄の有効状態を同期する */
export function syncSelectorWebviewIdleSection(): void {
    const section = document.getElementById('selector-webview-idle-section');
    if (section) {
        section.classList.toggle(
            'disabled',
            loadedState().selectorWebviewRetention !== 'DisposeAfterIdle'
        );
    }
}

/**
 * コンテンツ領域でフォーム要素が編集中かどうか
 */
export function isContentEditing(): boolean {
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
 */
function clampLimitValue(
    raw: string | number,
    min: number,
    max: number
): { value: number; clamped: boolean } {
    return SettingsSharedLogic.clampLimitValue(raw, min, max);
}

/**
 * フィールドに調整フィードバックを表示する
 */
function showFieldFeedback(input: HTMLElement, message: string): void {
    const field = input.closest('.field');
    if (!field) {
        return;
    }
    let feedback: HTMLElement | null = field.querySelector('.field-feedback');
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
 */
function clearFieldFeedback(input: HTMLElement): void {
    const field = input.closest('.field');
    if (!field) {
        return;
    }
    const feedback: HTMLElement | null = field.querySelector('.field-feedback');
    if (feedback) {
        feedback.hidden = true;
    }
    field.classList.remove('field-adjusted');
}

/**
 * 件数入力で更新する設定キー
 */
export type LimitKey =
    'intervalMs' | 'historyLimit' | 'continuousRecordLimit' | 'selectorWebviewIdleMinutes';

/**
 * 件数入力をコミットし state とパッチを更新する
 *
 * @param patchKey - `scheduleApply` に渡すキー
 * @param stateKey - `state` 上のキー
 */
export function commitLimitInput(
    input: HTMLInputElement,
    patchKey: LimitKey,
    stateKey: LimitKey,
    bounds: { min: number; max: number }
): void {
    const result = clampLimitValue(input.value, bounds.min, bounds.max);
    input.value = String(result.value);
    if (result.clamped) {
        showFieldFeedback(input, bounds.min + '〜' + bounds.max + ' の範囲に調整しました');
    } else {
        clearFieldFeedback(input);
    }
    loadedState()[stateKey] = result.value;
    scheduleApply({ [patchKey]: result.value });
}

/**
 * 指定タブのパネルをキャッシュから破棄する
 */
export function invalidatePanel(tabId: string): void {
    ctx.renderedPanels.delete(tabId);
    ctx.boundPanels.delete(tabId);
    document.getElementById('panel-' + tabId)?.remove();
}

/** 全タブのパネルキャッシュを破棄しコンテンツを空にする */
export function invalidateAllPanels(): void {
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
 */
function setContentScrollTop(content: HTMLElement, top: number): void {
    content.scrollTo({ top, behavior: 'instant' });
}

/** アクティブタブのパネルを表示しスクロール位置を復元する */
export function showActivePanel(): void {
    const content = document.getElementById('content');
    if (!content) {
        return;
    }
    content.querySelectorAll<HTMLElement>('.panel').forEach((panel) => {
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
 * @param action - パネルを再描画する処理
 */
export function withPreservedScroll(action: () => void): void {
    const content = document.getElementById('content');
    const top = content ? content.scrollTop : 0;
    action();
    if (content) {
        ctx.tabScrollPositions[ctx.activeTab] = top;
        setContentScrollTop(content, top);
    }
}

/** サーバー主導のフィールド (自動変換・連続記録など) を DOM に反映する */
export function syncServerFields(): void {
    const launch = inputById('launch-at-login');
    if (launch) {
        launch.checked = loadedState().launchAtLogin;
    }
    const autoRefine = inputById('auto-refine-enabled');
    if (autoRefine) {
        autoRefine.checked = loadedState().autoRefineEnabled;
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
export function syncHotkeyInputsFromState(): void {
    const hotkeys: Partial<HotkeySettings> = loadedState().hotkeys || {};
    const active = document.activeElement;

    for (const [elementId, key] of SettingsSharedLogic.HOTKEY_FIXED_FIELD_MAP) {
        const element = inputById(elementId);
        if (!element || element === active) {
            continue;
        }
        const next = hotkeys[key] || '';
        if (element.value !== next) {
            element.value = next;
        }
        element.classList.remove('is-invalid');
    }

    const favoriteModes = loadedState().favoriteModes || [];
    const slots = Array.isArray(hotkeys.favoriteModeSlots) ? hotkeys.favoriteModeSlots : [];
    for (let index = 0; index < favoriteModes.length; index += 1) {
        const element = inputById('hotkey-favorite-slot-' + index);
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
 */
export function setClipRegisterPending(pending: boolean): void {
    ctx.clipRegisterPending = pending;
    updateClipRegisterButton();
}

/** クリップ登録ボタンの表示を `clipRegisterPending` に合わせて更新する */
function updateClipRegisterButton(): void {
    const btn = buttonById('clip-register-btn');
    if (!btn) {
        return;
    }
    btn.disabled = ctx.clipRegisterPending;
    btn.textContent = ctx.clipRegisterPending ? '登録中...' : 'クリップボードから登録';
    btn.classList.toggle('is-loading', ctx.clipRegisterPending);
}

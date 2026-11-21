// ======================================================================
// 設定画面 純粋ロジック (Node テスト・WebView 共有)
// ======================================================================

/**
 * DOM 非依存の設定 UI 純粋ロジック
 *
 * @namespace SettingsSharedLogic
 */
const SettingsSharedLogic = (function () {
    'use strict';

    /** @type {ReadonlyArray<readonly [string, string]>} 通知タブの DOM ID と設定キー */
    const NOTIFICATION_FIELD_MAP = [
        ['notify-enabled', 'enabled'],
        ['notify-process', 'notifyProcess'],
        ['notify-pause', 'notifyPause'],
        ['notify-history-restore', 'notifyHistoryRestore'],
        ['notify-ocr', 'notifyOcr'],
        ['notify-registered-clip', 'notifyRegisteredClip'],
        ['notify-favorites', 'notifyFavorites'],
        ['notify-continuous-record', 'notifyContinuousRecord'],
        ['notify-result', 'notifyResult'],
    ];

    /** @type {ReadonlyArray<readonly [string, string]>} 固定ホットキーの DOM ID と設定キー */
    const HOTKEY_FIXED_FIELD_MAP = [
        ['hotkey-quick-selector', 'quickSelector'],
        ['hotkey-clip-selector', 'clipSelector'],
        ['hotkey-ocr', 'ocr'],
        ['hotkey-notification', 'notification'],
        ['hotkey-pause', 'pause'],
        ['hotkey-continuous-record', 'continuousRecord'],
        ['hotkey-continuous-record-undo', 'continuousRecordUndo'],
        ['hotkey-continuous-record-paste', 'continuousRecordPaste'],
        ['hotkey-undo', 'undo'],
        ['hotkey-quit', 'quit'],
    ];

    /** @type {ReadonlyArray<readonly [string, string]>} 固定ホットキーの設定キーと表示ラベル */
    const HOTKEY_FIXED_LABELS = [
        ['quickSelector', 'クイックセレクター'],
        ['clipSelector', '登録クリップセレクター'],
        ['ocr', '画面 OCR'],
        ['notification', '成功通知の切替'],
        ['pause', '一時停止 / 再開'],
        ['continuousRecord', '連続記録の切替'],
        ['continuousRecordUndo', '連続記録の取り消し'],
        ['continuousRecordPaste', '連続記録の貼り付け'],
        ['undo', '加工の取り消し'],
        ['quit', '終了'],
    ];

    /**
     * 件数入力を min/max へクランプする
     *
     * @param {string|number} raw
     * @param {number} min
     * @param {number} max
     * @returns {{ value: number, clamped: boolean }}
     */
    function clampLimitValue(raw, min, max) {
        const parsed = Number.parseInt(String(raw), 10);
        const invalid = Number.isNaN(parsed);
        const value = invalid ? min : parsed;
        const bounded = Math.min(max, Math.max(min, value));
        return {
            value: bounded,
            clamped: invalid || bounded !== value,
        };
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
     * 指定モードが画像を出力するかどうか
     *
     * @param {string} modeId
     * @param {ReadonlySet<string>} imageModeIds
     * @returns {boolean}
     */
    function modeProducesImage(modeId, imageModeIds) {
        return imageModeIds.has(modeId);
    }

    /**
     * 加工パイプラインを正規化する
     *
     * 画像出力モードは末尾に1つのみ残し、最大段数を超える分は切り捨てる
     *
     * @param {string[]} pipeline
     * @param {number} maxPipelineLength
     * @param {ReadonlySet<string>} imageModeIds
     * @returns {string[]}
     */
    function normalizePipeline(pipeline, maxPipelineLength, imageModeIds) {
        let imageMode = null;
        for (let index = pipeline.length - 1; index >= 0; index -= 1) {
            if (modeProducesImage(pipeline[index], imageModeIds)) {
                imageMode = pipeline[index];
                break;
            }
        }
        let normalized = pipeline.filter((modeId) => !modeProducesImage(modeId, imageModeIds));
        if (imageMode !== null) {
            normalized.push(imageMode);
        }
        if (normalized.length > maxPipelineLength) {
            normalized = normalized.slice(0, maxPipelineLength);
        }
        return normalized;
    }

    /**
     * 通知タブのチェック状態から IPC パッチ用オブジェクトを組み立てる
     *
     * @param {Record<string, boolean>} fieldStates - DOM ID をキーとしたチェック状態
     * @returns {Record<string, boolean>}
     */
    function buildNotificationSettingsPatch(fieldStates) {
        const notificationSettings = {};
        for (const [elementId, key] of NOTIFICATION_FIELD_MAP) {
            if (Object.prototype.hasOwnProperty.call(fieldStates, elementId)) {
                notificationSettings[key] = fieldStates[elementId];
            }
        }
        return notificationSettings;
    }

    /**
     * ホットキー文字列がクライアント側で妥当か判定する
     *
     * 空文字はお気に入りスロットの無効化のみ許可する
     *
     * @param {string} binding
     * @param {{ allowEmpty?: boolean }} [options]
     * @returns {{ valid: boolean, message: string }}
     */
    function validateHotkeyBinding(binding, options) {
        const opts = options || {};
        const trimmed = String(binding || '').trim();
        if (!trimmed) {
            if (opts.allowEmpty) {
                return { valid: true, message: '' };
            }
            return { valid: false, message: 'ホットキーを入力してください' };
        }

        const parts = trimmed
            .split('+')
            .map((part) => part.trim())
            .filter(Boolean);
        if (parts.length === 0) {
            return { valid: false, message: 'ホットキーを入力してください' };
        }

        const modifiers = new Set(['alt', 'shift', 'ctrl', 'control', 'meta', 'super', 'win']);
        let keyPart = null;
        for (const part of parts) {
            const lower = part.toLowerCase();
            if (modifiers.has(lower)) {
                continue;
            }
            if (keyPart !== null) {
                return { valid: false, message: 'キーは1つだけ指定してください' };
            }
            keyPart = part;
        }
        if (keyPart === null) {
            return { valid: false, message: 'キーが指定されていません' };
        }

        const upper = keyPart.toUpperCase();
        const isLetter = /^[A-Z]$/.test(upper);
        const isDigit = /^[0-9]$/.test(upper);
        const isFunction = /^F([1-9]|1[0-2])$/i.test(keyPart);
        const isBackspace = upper === 'BACKSPACE';
        if (!isLetter && !isDigit && !isFunction && !isBackspace) {
            return {
                valid: false,
                message: '未対応のキーです (A-Z / 0-9 / F1-F12 / Backspace)',
            };
        }
        return { valid: true, message: '' };
    }

    /**
     * DOM / 状態から IPC 用のホットキーパッチを組み立てる
     *
     * @param {Record<string, string>} fixedFieldStates - DOM ID をキーとした固定ホットキー
     * @param {string[]} favoriteSlotBindings - お気に入りスロットのバインディング
     * @returns {Record<string, unknown>}
     */
    function buildHotkeysPatch(fixedFieldStates, favoriteSlotBindings) {
        const hotkeys = {
            favoriteModeSlots: Array.isArray(favoriteSlotBindings)
                ? favoriteSlotBindings.map((binding) => String(binding || '').trim())
                : [],
        };
        for (const [elementId, key] of HOTKEY_FIXED_FIELD_MAP) {
            if (Object.prototype.hasOwnProperty.call(fixedFieldStates, elementId)) {
                hotkeys[key] = String(fixedFieldStates[elementId] || '').trim();
            }
        }
        return hotkeys;
    }

    /**
     * 設定スナップショット JSON に必須のホットキーフィールドが含まれるか検証する
     *
     * @param {Record<string, unknown>} snapshot
     * @returns {boolean}
     */
    function snapshotHasHotkeyFields(snapshot) {
        const hotkeys = snapshot.hotkeys;
        if (!hotkeys || typeof hotkeys !== 'object') {
            return false;
        }
        const required = HOTKEY_FIXED_FIELD_MAP.map(([, key]) => key);
        required.push('favoriteModeSlots');
        return required.every((key) => Object.prototype.hasOwnProperty.call(hotkeys, key));
    }

    /**
     * 設定スナップショット JSON に必須の通知フィールドが含まれるか検証する
     *
     * @param {Record<string, unknown>} snapshot
     * @returns {boolean}
     */
    function snapshotHasNotificationFields(snapshot) {
        const notificationSettings = snapshot.notificationSettings;
        if (!notificationSettings || typeof notificationSettings !== 'object') {
            return false;
        }

        const required = [
            'enabled',
            'notifyProcess',
            'notifyPause',
            'notifyHistoryRestore',
            'notifyOcr',
            'notifyRegisteredClip',
            'notifyFavorites',
            'notifyContinuousRecord',
            'notifyResult',
        ];

        return required.every((key) =>
            Object.prototype.hasOwnProperty.call(notificationSettings, key)
        );
    }

    /**
     * スナップショット受信時にフル再描画が必要かどうか
     *
     * @param {{ force?: boolean, externalSync?: boolean }} options
     * @param {boolean} wasEditing - 受信時にフォーム編集中だったか
     * @param {string} activeTab - 現在のタブ ID
     * @param {boolean} clipRegisterPending - 登録クリップ処理中か
     * @returns {boolean}
     */
    function shouldFullRenderOnLoad(options, wasEditing, activeTab, clipRegisterPending) {
        const opts = options || {};
        if (opts.force || opts.externalSync) {
            return true;
        }
        if (clipRegisterPending && activeTab === 'clips') {
            return true;
        }
        if (!wasEditing) {
            return true;
        }
        // ショートカットタブは入力中フォーカスを維持するためフル再描画しない
        return activeTab === 'favorites';
    }

    /**
     * お気に入り/ショートカット差分更新時のショートカットタブの更新方針
     *
     * 入力中かつお気に入り件数が変わらない場合はフォーカス維持のため入力欄のみ同期する
     *
     * @param {{ wasEditingShortcuts: boolean, previousFavoriteCount: number, nextFavoriteCount: number }} options
     * @returns {'syncInputs'|'refreshPanel'}
     */
    function resolveShortcutsDiffUpdate(options) {
        const opts = options || {};
        if (opts.wasEditingShortcuts && opts.previousFavoriteCount === opts.nextFavoriteCount) {
            return 'syncInputs';
        }
        return 'refreshPanel';
    }

    /**
     * Escape キーで設定ウィンドウを閉じられるか判定する
     *
     * 確認ダイアログ表示中やフォーム編集中は閉じない
     *
     * @param {{ dialogVisible: boolean, isEditing: boolean }} options
     * @returns {boolean}
     */
    function shouldCloseSettingsOnEscape(options) {
        const opts = options || {};
        return !opts.dialogVisible && !opts.isEditing;
    }

    /**
     * 閉じる操作で送る IPC メッセージ列を組み立てる
     *
     * 保留パッチがあれば先に `apply` し、続けて `close` を送る
     *
     * @param {Record<string, unknown>|null|undefined} pendingPatch
     * @returns {Array<{ type: string, patch?: Record<string, unknown> }>}
     */
    function collectCloseWindowIpcMessages(pendingPatch) {
        const messages = [];
        const patch =
            pendingPatch && typeof pendingPatch === 'object' && !Array.isArray(pendingPatch)
                ? pendingPatch
                : {};
        if (Object.keys(patch).length > 0) {
            messages.push({ type: 'apply', patch: patch });
        }
        messages.push({ type: 'close' });
        return messages;
    }

    /**
     * 設定シェル HTML に必須の DOM ID が含まれるか検証する
     *
     * @param {string} html
     * @returns {boolean}
     */
    function settingsShellHasRequiredIds(html) {
        const required = [
            'id="app"',
            'id="sidebar"',
            'id="content"',
            'id="open-config-btn"',
            'id="close-btn"',
            'id="apply-feedback"',
            'id="confirm-dialog"',
        ];
        return required.every((marker) => html.includes(marker));
    }

    /** @type {ReadonlyArray<string>} 設定ウィンドウのタブ ID */
    const SETTINGS_TAB_IDS = [
        'general',
        'favorites',
        'regex',
        'history',
        'clips',
        'monitor',
        'notifications',
        'shortcuts',
    ];

    /**
     * 組み立て済み設定 JS に全タブ ID が含まれるか検証する
     *
     * esbuild バンドルはダブルクォート、ソース連結はシングルクォートの場合がある
     *
     * @param {string} js
     * @returns {boolean}
     */
    function settingsJsDefinesAllTabs(js) {
        return SETTINGS_TAB_IDS.every(
            (tabId) => js.includes("id: '" + tabId + "'") || js.includes('id: "' + tabId + '"')
        );
    }

    return {
        NOTIFICATION_FIELD_MAP,
        HOTKEY_FIXED_FIELD_MAP,
        HOTKEY_FIXED_LABELS,
        SETTINGS_TAB_IDS,
        clampLimitValue,
        escapeHtml,
        modeProducesImage,
        normalizePipeline,
        buildNotificationSettingsPatch,
        validateHotkeyBinding,
        buildHotkeysPatch,
        snapshotHasNotificationFields,
        snapshotHasHotkeyFields,
        shouldFullRenderOnLoad,
        resolveShortcutsDiffUpdate,
        shouldCloseSettingsOnEscape,
        collectCloseWindowIpcMessages,
        settingsShellHasRequiredIds,
        settingsJsDefinesAllTabs,
    };
})();

export default SettingsSharedLogic;

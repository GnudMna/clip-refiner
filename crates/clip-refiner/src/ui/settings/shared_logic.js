// ======================================================================
// 設定画面 純粋ロジック (Node テスト・WebView 共有)
// ======================================================================

/**
 * DOM 非依存の設定 UI 純粋ロジック
 *
 * @namespace SettingsSharedLogic
 */
var SettingsSharedLogic = (function () {
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

    return {
        NOTIFICATION_FIELD_MAP,
        clampLimitValue,
        escapeHtml,
        modeProducesImage,
        normalizePipeline,
        buildNotificationSettingsPatch,
        snapshotHasNotificationFields,
    };
})();

if (typeof module !== 'undefined' && module.exports) {
    module.exports = SettingsSharedLogic;
}

// ======================================================================
// 設定パッチ (純粋ロジック)
// ======================================================================

/**
 * 設定画面の部分更新オブジェクトをマージする純粋ロジック
 *
 * @namespace SettingsPatch
 */
var SettingsPatch = (function () {
    'use strict';

    /**
     * 部分更新オブジェクトを深くマージする
     *
     * ネストしたプレーンオブジェクトは再帰的にマージし、配列とプリミティブは上書きする
     *
     * @param {Record<string, unknown>} target - マージ先
     * @param {Record<string, unknown>} patch - 適用する差分
     * @returns {Record<string, unknown>} マージ後の `target` (同一参照)
     */
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

    return {
        mergePatch,
    };
})();

if (typeof module !== 'undefined' && module.exports) {
    module.exports = SettingsPatch;
}

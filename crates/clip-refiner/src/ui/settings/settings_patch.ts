// ======================================================================
// 設定パッチ (純粋ロジック)
// ======================================================================

/**
 * 設定画面の部分更新オブジェクトをマージする純粋ロジック
 */
const SettingsPatch = (function () {
    'use strict';

    /**
     * 部分更新オブジェクトを深くマージする
     *
     * ネストしたプレーンオブジェクトは再帰的にマージし、配列とプリミティブは上書きする
     * `null` / `undefined` は未指定として扱い、既存値を維持する (Rust 側 `deep_merge_json_values` と同仕様)
     *
     * @param target - マージ先
     * @param patch - 適用する差分
     * @returns マージ後の `target` (同一参照)
     */
    function mergePatch(
        target: Record<string, unknown>,
        patch: Record<string, unknown>
    ): Record<string, unknown> {
        for (const [key, value] of Object.entries(patch)) {
            if (value === null || value === undefined) {
                continue;
            }
            if (typeof value === 'object' && !Array.isArray(value)) {
                const current = target[key];
                const nested =
                    current && typeof current === 'object' && !Array.isArray(current)
                        ? (current as Record<string, unknown>)
                        : {};
                target[key] = mergePatch(nested, value as Record<string, unknown>);
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

export default SettingsPatch;

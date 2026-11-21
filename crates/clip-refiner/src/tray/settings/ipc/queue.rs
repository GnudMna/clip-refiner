use std::sync::Arc;

use super::super::model::{SettingsPatch, deep_merge_json_values};

use crate::tray::state::AppState;

// ======================================================================
// 適用待ちキュー
// ======================================================================
/// 適用待ちキューへ部分更新 JSON を原子的にマージして格納する
///
/// マージに失敗した場合は既存キューを保持し、`false` を返す
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn enqueue_settings_patch_json(state: &Arc<AppState>, patch_json: &str) -> bool {
    let mut guard = state.settings_pending().patch_mut();
    let merged_json = if let Some(existing) = guard.as_ref() {
        match merge_settings_patch_json_strings(existing, patch_json) {
            Some(merged) => merged,
            None => return false,
        }
    } else {
        // 単体投入でも不正 JSON は受け付けない
        let parsed: serde_json::Value = match serde_json::from_str(patch_json) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if !parsed.is_object() {
            return false;
        }
        match serde_json::to_string(&parsed) {
            Ok(json) => json,
            Err(_) => return false,
        }
    };
    *guard = Some(merged_json);
    true
}

/// 適用待ちキューへ部分更新を原子的にマージして格納する
///
/// マージに失敗した場合は既存キューを保持し、`false` を返す
pub(crate) fn enqueue_settings_patch(state: &Arc<AppState>, patch: &SettingsPatch) -> bool {
    let Ok(incoming_json) = serde_json::to_string(patch) else {
        return false;
    };

    let mut guard = state.settings_pending().patch_mut();
    let merged_json = if let Some(existing) = guard.as_ref() {
        match merge_settings_patch_json_strings(existing, &incoming_json) {
            Some(merged) => merged,
            None => return false,
        }
    } else {
        incoming_json
    };
    *guard = Some(merged_json);
    true
}

/// 2 つの部分更新 JSON 文字列を深くマージする
pub(super) fn merge_settings_patch_json_strings(existing: &str, incoming: &str) -> Option<String> {
    let mut base = serde_json::from_str::<serde_json::Value>(existing).ok()?;
    let incoming_value = serde_json::from_str::<serde_json::Value>(incoming).ok()?;
    deep_merge_json_values(&mut base, &incoming_value);
    serde_json::to_string(&base).ok()
}

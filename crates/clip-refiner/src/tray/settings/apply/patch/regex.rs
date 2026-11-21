use std::sync::Arc;

use super::super::feedback::SettingsApplyFeedback;
use crate::refiner::{REGEX_PREVIEW_SAMPLE, preview_regex_replace};
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 正規表現パッチ
// ======================================================================
/// 正規表現設定のパッチを適用する
///
/// パターンがコンパイルできない場合は設定を更新せずエラーフィードバックを返す
pub(super) fn apply_regex_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    config_dirty: &mut bool,
) -> Option<SettingsApplyFeedback> {
    let regex = patch.regex.as_ref()?;

    let preview = preview_regex_replace(REGEX_PREVIEW_SAMPLE, regex);
    if !preview.valid {
        return Some(SettingsApplyFeedback::error(Some("regex"), preview.message));
    }

    state.with_config_mut(|c| c.regex = regex.clone());
    *config_dirty = true;
    None
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::tray::state::test_app_state;

    /// 正規表現パッチが `AppConfig` へ反映されるようにすること
    #[test]
    fn regex_patch_updates_config() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings {
                pattern: r"\d+".to_string(),
                replacement: "bar".to_string(),
                case_insensitive: true,
                multiline: true,
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(apply_regex_patch(&patch, &state, &mut config_dirty).is_none());

        let regex = state.with_config(|c| c.regex.clone());
        assert_eq!(regex.pattern, r"\d+");
        assert_eq!(regex.replacement, "bar");
        assert!(regex.case_insensitive);
        assert!(regex.multiline);
        assert!(config_dirty);
    }

    /// 無効な正規表現パッチは拒否されるようにすること
    #[test]
    fn regex_patch_rejects_invalid_pattern() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings {
                pattern: "[unclosed".to_string(),
                replacement: String::new(),
                ..crate::config::RegexSettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let feedback = apply_regex_patch(&patch, &state, &mut config_dirty)
            .expect("無効パターンでエラーフィードバックが返ること");
        assert!(!feedback.ok);
        assert_eq!(feedback.field.as_deref(), Some("regex"));
        assert!(!config_dirty);
    }
}

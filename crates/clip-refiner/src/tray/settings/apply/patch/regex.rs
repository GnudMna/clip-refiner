use std::sync::Arc;

use super::super::feedback::SettingsApplyFeedback;
use crate::consts::{MAX_REGEX_PRESETS, MAX_REGEX_STEPS_PER_PRESET};
use crate::refiner::validate_regex_settings;
use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 正規表現パッチ
// ======================================================================
/// 正規表現設定のパッチを適用する
///
/// いずれかのステップがコンパイルできない場合は設定を更新せずエラーフィードバックを返す
pub(super) fn apply_regex_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    config_dirty: &mut bool,
) -> Option<SettingsApplyFeedback> {
    let regex = patch.regex.as_ref()?;

    if regex.presets.len() > MAX_REGEX_PRESETS {
        return Some(SettingsApplyFeedback::error(
            Some("regex"),
            format!("正規表現プリセットは最大 {MAX_REGEX_PRESETS} 件までです"),
        ));
    }
    if regex
        .presets
        .iter()
        .any(|preset| preset.steps.len() > MAX_REGEX_STEPS_PER_PRESET)
    {
        return Some(SettingsApplyFeedback::error(
            Some("regex"),
            format!("各プリセットのステップは最大 {MAX_REGEX_STEPS_PER_PRESET} 件までです"),
        ));
    }

    let preview = validate_regex_settings(regex);
    if !preview.valid {
        return Some(SettingsApplyFeedback::error(Some("regex"), preview.message));
    }

    let mut normalized = regex.clone();
    normalized.normalize();
    state.with_config_mut(|c| c.regex = normalized);
    *config_dirty = true;
    menu.refresh_regex_presets_logged(state);
    None
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::{RegexPreset, RegexSettings, RegexStep};
    use crate::tray::state::test_app_state;

    /// 正規表現パッチが `AppConfig` へ反映されるようにすること
    #[test]
    fn regex_patch_updates_config() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            regex: Some(RegexSettings::from_step(RegexStep {
                pattern: r"\d+".to_string(),
                replacement: "bar".to_string(),
                case_insensitive: true,
                multiline: true,
            })),
            ..SettingsPatch::default()
        };

        let ctx = crate::test_helpers::TrayTestContext::from_state(state);
        let mut config_dirty = false;
        assert!(apply_regex_patch(&patch, ctx.state(), ctx.menu(), &mut config_dirty).is_none());

        let regex = ctx.state().with_config(|c| c.regex.clone());
        let step = &regex.active_steps()[0];
        assert_eq!(step.pattern, r"\d+");
        assert_eq!(step.replacement, "bar");
        assert!(step.case_insensitive);
        assert!(step.multiline);
        assert!(config_dirty);
    }

    /// 無効な正規表現パッチは拒否されるようにすること
    #[test]
    fn regex_patch_rejects_invalid_pattern() {
        let state = Arc::new(test_app_state());
        let ctx = crate::test_helpers::TrayTestContext::from_state(state);
        let patch = SettingsPatch {
            regex: Some(RegexSettings::single("[unclosed", "")),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let feedback = apply_regex_patch(&patch, ctx.state(), ctx.menu(), &mut config_dirty)
            .expect("無効パターンでエラーフィードバックが返ること");
        assert!(!feedback.ok);
        assert_eq!(feedback.field.as_deref(), Some("regex"));
        assert!(!config_dirty);
    }

    /// ステップ数上限を超えるパッチは拒否すること
    #[test]
    fn regex_patch_rejects_too_many_steps() {
        let state = Arc::new(test_app_state());
        let ctx = crate::test_helpers::TrayTestContext::from_state(state);
        let steps = (0..=MAX_REGEX_STEPS_PER_PRESET)
            .map(|_| RegexStep::default())
            .collect();
        let patch = SettingsPatch {
            regex: Some(RegexSettings {
                active_preset_id: "default".to_string(),
                presets: vec![RegexPreset {
                    id: "default".to_string(),
                    name: "デフォルト".to_string(),
                    steps,
                }],
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let feedback = apply_regex_patch(&patch, ctx.state(), ctx.menu(), &mut config_dirty)
            .expect("上限超過でエラーになること");
        assert!(!feedback.ok);
        assert!(!config_dirty);
    }
}

use std::sync::Arc;

use super::super::feedback::SettingsApplyFeedback;
use crate::config::format_hotkey_fixes_message;
use crate::tray::dispatch;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::{AppState, HotkeyEvent};

// ======================================================================
// ホットキーパッチ
// ======================================================================
/// ホットキー設定のパッチを適用する
///
/// 不正・重複は `fix_invalid` で正規化し、OS 再登録は `HotkeyEvent::ReloadAll` で行う
/// 正規化が発生した場合は設定 UI 向けのフィードバックを返す
pub(super) fn apply_hotkeys_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    config_dirty: &mut bool,
) -> HotkeysPatchOutcome {
    let Some(hotkeys) = patch.hotkeys.as_ref() else {
        return HotkeysPatchOutcome::default();
    };

    let fixes = state.with_config_mut(|c| {
        c.hotkeys = hotkeys.clone();
        c.hotkeys.fix_invalid()
    });
    *config_dirty = true;
    dispatch::send_app_event(&state.proxy, HotkeyEvent::ReloadAll.into());

    let feedback = format_hotkey_fixes_message(&fixes)
        .map(|message| SettingsApplyFeedback::ok(message).with_field(Some("hotkeys")));

    HotkeysPatchOutcome {
        changed: true,
        feedback,
    }
}

/// ホットキーパッチ適用の結果
#[derive(Debug, Default)]
pub(super) struct HotkeysPatchOutcome {
    /// 設定が更新されたかどうか
    pub(super) changed: bool,
    /// 正規化が発生した場合のユーザー向けフィードバック
    pub(super) feedback: Option<SettingsApplyFeedback>,
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::HotkeySettings;
    use crate::consts;
    use crate::tray::state::test_app_state;

    /// ホットキーパッチが `AppConfig` へ反映されるようにすること
    #[test]
    fn hotkeys_patch_updates_config() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            hotkeys: Some(HotkeySettings {
                quick_selector: "Ctrl+Shift+S".to_string(),
                ..HotkeySettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let outcome = apply_hotkeys_patch(&patch, &state, &mut config_dirty);
        assert!(outcome.changed);
        assert!(outcome.feedback.is_none());
        assert_eq!(
            state.with_config(|c| c.hotkeys.quick_selector.clone()),
            "Ctrl+Shift+S"
        );
        assert!(config_dirty);
    }

    /// 不正な固定ホットキーはデフォルトへ正規化しフィードバックを返すようにすること
    #[test]
    fn hotkeys_patch_fixes_invalid_binding_with_feedback() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            hotkeys: Some(HotkeySettings {
                auto_refine: "Bad+Key".to_string(),
                ..HotkeySettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let outcome = apply_hotkeys_patch(&patch, &state, &mut config_dirty);
        assert!(outcome.changed);
        let feedback = outcome.feedback.expect("正規化フィードバック");
        assert!(feedback.ok);
        assert_eq!(feedback.field.as_deref(), Some("hotkeys"));
        assert!(feedback.message.contains("自動変換の切替"));
        assert_eq!(
            state.with_config(|c| c.hotkeys.auto_refine.clone()),
            consts::DEFAULT_HOTKEY_AUTO_REFINE
        );
    }

    /// 重複するホットキーは正規化しフィードバックを返すようにすること
    #[test]
    fn hotkeys_patch_fixes_duplicate_binding_with_feedback() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            hotkeys: Some(HotkeySettings {
                notification: "Ctrl+Shift+N".to_string(),
                auto_refine: "Ctrl+Shift+N".to_string(),
                ..HotkeySettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let outcome = apply_hotkeys_patch(&patch, &state, &mut config_dirty);
        let feedback = outcome.feedback.expect("重複フィードバック");
        assert!(feedback.ok);
        assert!(feedback.message.contains("自動変換の切替"));
        assert!(feedback.message.contains("成功通知の切替"));
        assert_eq!(
            state.with_config(|c| c.hotkeys.auto_refine.clone()),
            consts::DEFAULT_HOTKEY_AUTO_REFINE
        );
    }

    /// ホットキーパッチが無い場合は何もしないようにすること
    #[test]
    fn hotkeys_patch_skips_when_absent() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch::default();
        let mut config_dirty = false;
        let outcome = apply_hotkeys_patch(&patch, &state, &mut config_dirty);
        assert!(!outcome.changed);
        assert!(outcome.feedback.is_none());
        assert!(!config_dirty);
    }
}

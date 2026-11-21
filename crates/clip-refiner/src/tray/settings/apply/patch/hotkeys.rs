use std::sync::Arc;

use crate::tray::dispatch;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::{AppEvent, AppState};

// ======================================================================
// ホットキーパッチ
// ======================================================================
/// ホットキー設定のパッチを適用する
///
/// 不正・重複は `fix_invalid` で正規化し、OS 再登録は `ReloadHotkeys` で行う
pub(super) fn apply_hotkeys_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    config_dirty: &mut bool,
) -> bool {
    let Some(hotkeys) = patch.hotkeys.as_ref() else {
        return false;
    };

    state.with_config_mut(|c| {
        c.hotkeys = hotkeys.clone();
        c.hotkeys.fix_invalid();
    });
    *config_dirty = true;
    dispatch::send_app_event(&state.proxy, AppEvent::ReloadHotkeys);
    true
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
        assert!(apply_hotkeys_patch(&patch, &state, &mut config_dirty));
        assert_eq!(
            state.with_config(|c| c.hotkeys.quick_selector.clone()),
            "Ctrl+Shift+S"
        );
        assert!(config_dirty);
    }

    /// 不正な固定ホットキーはデフォルトへ正規化されるようにすること
    #[test]
    fn hotkeys_patch_fixes_invalid_binding() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            hotkeys: Some(HotkeySettings {
                pause: "Bad+Key".to_string(),
                ..HotkeySettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(apply_hotkeys_patch(&patch, &state, &mut config_dirty));
        assert_eq!(
            state.with_config(|c| c.hotkeys.pause.clone()),
            consts::DEFAULT_HOTKEY_PAUSE
        );
    }

    /// ホットキーパッチが無い場合は何もしないようにすること
    #[test]
    fn hotkeys_patch_skips_when_absent() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch::default();
        let mut config_dirty = false;
        assert!(!apply_hotkeys_patch(&patch, &state, &mut config_dirty));
        assert!(!config_dirty);
    }
}

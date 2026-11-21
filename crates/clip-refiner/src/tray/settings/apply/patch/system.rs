use std::sync::Arc;

use super::super::feedback::SettingsApplyFeedback;

use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::menu::TrayMenu;
use crate::tray::notify;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;
use crate::user_error::{UserNotification, titles};

// ======================================================================
// OS 連携パッチ
// ======================================================================
/// OS 連携パッチの適用結果
pub(super) struct SystemPatchOutcome {
    /// UI の強制再同期が必要かどうか
    pub(super) force_refresh: bool,
    /// ユーザーへ通知するフィードバック
    pub(super) feedback: Option<SettingsApplyFeedback>,
}

/// OS 連携と一時停止のパッチを適用する
pub(super) fn apply_system_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    config_dirty: &mut bool,
) -> SystemPatchOutcome {
    let mut force_refresh = false;
    let mut feedback = None;

    if let Some(launch_at_login) = patch.launch_at_login {
        let current = crate::platform::autostart::is_enabled();
        if current != launch_at_login
            && let Err(e) = crate::platform::autostart::set_enabled(launch_at_login)
        {
            UserNotification::static_message(
                titles::SETTINGS,
                "ログイン時自動起動の設定に失敗しました",
            )
            .notify_with_debug(&e);
            force_refresh = true;
            feedback = Some(
                SettingsApplyFeedback::error(
                    Some("launchAtLogin"),
                    "ログイン時自動起動の設定に失敗しました",
                )
                .with_desktop_already_notified(),
            );
        }
    }

    if let Some(is_paused) = patch.is_paused {
        let current = state.with_config(|c| c.is_paused);
        if current != is_paused {
            state.with_config_mut(|c| c.is_paused = is_paused);
            menu.pause_item.set_checked(is_paused);
            notify::show_pause_notification(state, is_paused, "設定ウィンドウ");
            *config_dirty = true;
            bump_monitor_generation(state);
        }
    }

    SystemPatchOutcome {
        force_refresh,
        feedback,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use crate::tray::menu::TrayMenu;
    use crate::tray::state::test_app_state;

    /// 一時停止パッチが `AppConfig` とトレイメニューへ反映されるようにすること
    #[test]
    fn pause_patch_updates_config_and_menu() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            is_paused: Some(true),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(!apply_system_patch(&patch, &state, &menu, &mut config_dirty).force_refresh);
        assert!(state.with_config(|c| c.is_paused));
        assert!(menu.pause_item.is_checked());
        assert!(config_dirty);
    }
}

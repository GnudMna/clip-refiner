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
    use super::*;
    use crate::test_helpers::TrayTestContext;

    /// 一時停止パッチが `AppConfig` とトレイメニューへ反映されるようにすること
    #[test]
    fn pause_patch_updates_config_and_menu() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            is_paused: Some(true),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(
            !apply_system_patch(&patch, ctx.state(), ctx.menu(), &mut config_dirty).force_refresh
        );
        assert!(ctx.state().with_config(|c| c.is_paused));
        assert!(ctx.menu().pause_item.is_checked());
        assert!(config_dirty);
    }

    /// 既に同じ一時停止状態なら no-op となるようにすること
    #[test]
    fn pause_patch_noop_when_unchanged() {
        let ctx = TrayTestContext::new();
        let current = ctx.state().with_config(|c| c.is_paused);
        let patch = SettingsPatch {
            is_paused: Some(current),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(
            !apply_system_patch(&patch, ctx.state(), ctx.menu(), &mut config_dirty).force_refresh
        );
        assert!(!config_dirty);
    }
}

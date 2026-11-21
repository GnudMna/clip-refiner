use super::super::feedback::SettingsApplyFeedback;
use crate::tray::settings::ipc::SettingsPatch;
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

/// OS 連携のパッチを適用する
pub(super) fn apply_system_patch(patch: &SettingsPatch) -> SystemPatchOutcome {
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

    SystemPatchOutcome {
        force_refresh,
        feedback,
    }
}

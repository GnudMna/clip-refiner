mod continuous_record;
mod history;
mod hotkeys;
mod monitor;
mod notification;
mod outcome;
mod refine;
mod regex;
mod selector_webview;
mod system;

use std::sync::Arc;

use super::feedback::SettingsPatchApplyResult;
use super::refresh::{SettingsRefreshScope, refresh_settings_window_scoped};
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::settings::SettingsWindow;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;
use crate::tray::worker::ClipboardWorkerHandle;
use continuous_record::apply_continuous_record_patch;
use history::apply_history_patch;
use hotkeys::apply_hotkeys_patch;
use monitor::apply_monitor_patch;
use notification::apply_notification_patch;
use outcome::PatchApplyOutcome;
use refine::apply_refine_patch;
use regex::apply_regex_patch;
use selector_webview::apply_selector_webview_patch;
use system::apply_system_patch;

// ======================================================================
// パッチ適用
// ======================================================================
/// 部分更新パッチを設定へ反映する
pub(super) fn apply_settings_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    settings: &SettingsWindow,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) {
    let result =
        apply_settings_patch_without_ui(patch, state, menu, quick_selector, clipboard_worker);
    if let Some(feedback) = result.feedback {
        settings.notify_apply_feedback(&feedback);
    }
    if let Some((scope, force)) = result.refresh {
        refresh_settings_window_scoped(state, settings, scope, force, false);
    }
}

/// UI を介さず部分更新パッチを設定へ反映する
pub(crate) fn apply_settings_patch_without_ui(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> SettingsPatchApplyResult {
    let mut outcome = PatchApplyOutcome {
        monitor_mode_changed: false,
        bump_monitor: false,
        mode_changed: false,
        pipeline_changed: false,
    };
    let mut config_dirty = false;

    apply_monitor_patch(patch, state, &mut outcome, &mut config_dirty);
    apply_selector_webview_patch(patch, state, menu, &mut outcome, &mut config_dirty);
    apply_notification_patch(patch, state, &mut config_dirty);
    apply_history_patch(patch, state, menu, &mut outcome, &mut config_dirty);
    apply_continuous_record_patch(
        patch,
        state,
        menu,
        clipboard_worker,
        &mut outcome,
        &mut config_dirty,
    );
    let regex_feedback = apply_regex_patch(patch, state, menu, &mut config_dirty);
    let hotkeys_outcome = apply_hotkeys_patch(patch, state, &mut config_dirty);
    apply_refine_patch(
        patch,
        state,
        menu,
        quick_selector,
        clipboard_worker,
        &mut outcome,
        &mut config_dirty,
    );
    let system_outcome = apply_system_patch(patch);

    if config_dirty {
        state.save_config();
    }

    if outcome.bump_monitor && !outcome.mode_changed {
        bump_monitor_generation(state);
    }

    // 正規表現エラーを優先し、次にホットキー正規化、最後にシステム系
    let feedback = regex_feedback
        .or(hotkeys_outcome.feedback)
        .or(system_outcome.feedback);

    SettingsPatchApplyResult {
        refresh: refresh_plan_after_patch(
            patch,
            system_outcome.force_refresh,
            hotkeys_outcome.changed,
        ),
        feedback,
    }
}

/// パッチ適用後に UI へ再同期が必要か判定する
///
/// UI 発の変更でサーバー側正規化 (`normalize` / `fix_invalid`) が走る項目のみ再同期する
pub(super) fn refresh_plan_after_patch(
    patch: &SettingsPatch,
    force: bool,
    hotkeys_changed: bool,
) -> Option<(SettingsRefreshScope, bool)> {
    if force {
        return Some((SettingsRefreshScope::Full, true));
    }
    if hotkeys_changed || patch.hotkeys.is_some() {
        return Some((SettingsRefreshScope::FavoritesAndShortcuts, false));
    }
    if patch.pipeline.is_some()
        || patch.history_limit.is_some()
        || patch.continuous_record_limit.is_some()
    {
        return Some((SettingsRefreshScope::Full, false));
    }
    None
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::feedback::should_notify_apply_error_via_desktop;
    use super::*;
    use crate::config::NotificationSettings;
    use crate::refiner::RefineMode;
    use crate::test_helpers::TrayTestContext;

    /// `apply_settings_patch_without_ui` が IPC 相当の変更を一括反映すること
    #[test]
    fn apply_without_ui_updates_config_from_merged_patch() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            interval_ms: Some(2_000),
            history_enabled: Some(true),
            ..SettingsPatch::default()
        };

        assert!(
            apply_settings_patch_without_ui(&patch, ctx.state(), ctx.menu(), None, ctx.worker(),)
                .refresh
                .is_none()
        );
        assert_eq!(ctx.state().with_config(|c| c.interval_ms), 2_000);
        assert!(ctx.state().with_config(|c| c.history_enabled));
        assert!(ctx.menu().history.enabled_item.is_checked());
    }

    /// ウィンドウ非表示後の無効な正規表現パッチがフォールバック通知対象となるようにすること
    #[test]
    fn invalid_regex_feedback_targets_desktop_fallback_after_hide() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings::single("[unclosed", "")),
            ..SettingsPatch::default()
        };

        let result =
            apply_settings_patch_without_ui(&patch, ctx.state(), ctx.menu(), None, ctx.worker());
        let feedback = result
            .feedback
            .expect("無効パターンでエラーフィードバックが返ること");
        assert!(!feedback.ok);
        assert!(!feedback.desktop_already_notified);
        assert!(should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 通知設定のみのパッチでは UI 再同期を省略すること
    #[test]
    fn refresh_plan_skips_notification_only_patch() {
        let patch = SettingsPatch {
            notification_settings: Some(NotificationSettings::default()),
            ..SettingsPatch::default()
        };
        assert!(refresh_plan_after_patch(&patch, false, false).is_none());
    }

    /// パイプライン変更では正規化結果を UI へ反映すること
    #[test]
    fn refresh_plan_syncs_pipeline_patch() {
        let patch = SettingsPatch {
            pipeline: Some(vec![RefineMode::Trim]),
            ..SettingsPatch::default()
        };
        assert_eq!(
            refresh_plan_after_patch(&patch, false, false),
            Some((SettingsRefreshScope::Full, false))
        );
    }

    /// 履歴件数変更では正規化結果を UI へ反映すること
    #[test]
    fn refresh_plan_syncs_history_limit_patch() {
        let patch = SettingsPatch {
            history_limit: Some(50),
            ..SettingsPatch::default()
        };
        assert_eq!(
            refresh_plan_after_patch(&patch, false, false),
            Some((SettingsRefreshScope::Full, false))
        );
    }

    /// ホットキー変更ではお気に入り・ショートカットを再同期すること
    #[test]
    fn refresh_plan_syncs_hotkeys_patch() {
        let patch = SettingsPatch {
            hotkeys: Some(crate::config::HotkeySettings::default()),
            ..SettingsPatch::default()
        };
        assert_eq!(
            refresh_plan_after_patch(&patch, false, true),
            Some((SettingsRefreshScope::FavoritesAndShortcuts, false))
        );
    }

    /// ホットキー正規化時に設定 UI 向けフィードバックを返すようにすること
    #[test]
    fn apply_without_ui_returns_hotkey_normalization_feedback() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            hotkeys: Some(crate::config::HotkeySettings {
                auto_refine: "Bad+Key".to_string(),
                ..crate::config::HotkeySettings::default()
            }),
            ..SettingsPatch::default()
        };

        let result =
            apply_settings_patch_without_ui(&patch, ctx.state(), ctx.menu(), None, ctx.worker());
        let feedback = result
            .feedback
            .expect("不正ホットキーで正規化フィードバックが返ること");
        assert!(feedback.ok);
        assert_eq!(feedback.field.as_deref(), Some("hotkeys"));
        assert!(feedback.message.contains("自動変換の切替"));
        assert_eq!(
            result.refresh,
            Some((SettingsRefreshScope::FavoritesAndShortcuts, false))
        );
    }

    /// 自動起動設定失敗時は強制再同期すること
    #[test]
    fn refresh_plan_forces_on_autostart_failure() {
        let patch = SettingsPatch::default();
        assert_eq!(
            refresh_plan_after_patch(&patch, true, false),
            Some((SettingsRefreshScope::Full, true))
        );
    }
}

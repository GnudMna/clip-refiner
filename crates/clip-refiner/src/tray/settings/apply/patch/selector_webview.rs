use std::sync::Arc;

use super::outcome::PatchApplyOutcome;
use crate::consts;
use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// セレクター WebView 保持方針
// ======================================================================
/// セレクター `WebView` 保持方針パッチを適用する
///
/// 実際の破棄同期はイベントループ側の `sync_selector_retention` が担う
pub(super) fn apply_selector_webview_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    _menu: &TrayMenu,
    _outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(retention) = patch.selector_webview_retention {
        let current = state.with_config(|c| c.selector_webview_retention);
        if current != retention {
            state.with_config_mut(|c| c.selector_webview_retention = retention);
            *config_dirty = true;
        }
    }

    if let Some(idle_minutes) = patch.selector_webview_idle_minutes {
        let clamped = idle_minutes.clamp(
            consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES,
            consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES,
        );
        let current = state.with_config(|c| c.selector_webview_idle_minutes);
        if current != clamped {
            state.with_config_mut(|c| c.selector_webview_idle_minutes = clamped);
            *config_dirty = true;
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SelectorWebviewRetention;
    use crate::test_helpers::TrayTestContext;

    /// 保持方針パッチが `AppConfig` へ反映されるようにすること
    #[test]
    fn retention_patch_updates_config() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            selector_webview_retention: Some(SelectorWebviewRetention::DisposeOnHide),
            ..SettingsPatch::default()
        };

        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;
        apply_selector_webview_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.selector_webview_retention),
            SelectorWebviewRetention::DisposeOnHide
        );
        assert!(config_dirty);
    }

    /// アイドル分数パッチが範囲内へクランプされるようにすること
    #[test]
    fn idle_minutes_patch_clamps() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            selector_webview_idle_minutes: Some(0),
            ..SettingsPatch::default()
        };

        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;
        apply_selector_webview_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.selector_webview_idle_minutes),
            consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES
        );
        assert!(config_dirty);
    }
}

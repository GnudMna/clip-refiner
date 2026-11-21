use std::sync::Arc;

use super::outcome::PatchApplyOutcome;

use crate::config::MonitorMode;
use crate::tray::event::update_monitor_mode;
use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 監視パッチ
// ======================================================================
/// 監視方式と監視周期のパッチを適用する
///
/// イベント監視への切替と同時に `interval_ms` が含まれる場合は周期をスキップする
/// (イベント監視時は周期を使用しない)
pub(super) fn apply_monitor_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(monitor_mode) = patch.monitor_mode
        && state.with_config(|c| c.monitor_mode) != monitor_mode
    {
        update_monitor_mode(state, menu, monitor_mode);
        outcome.monitor_mode_changed = true;
    }

    let switching_to_event =
        outcome.monitor_mode_changed && patch.monitor_mode == Some(MonitorMode::Event);

    if let Some(interval_ms) = patch.interval_ms
        && !switching_to_event
    {
        state.with_config_mut(|c| {
            c.interval_ms = interval_ms;
            c.normalize();
        });
        let interval_ms = state.with_config(|c| c.interval_ms);
        menu.sync_interval_menu(interval_ms);
        *config_dirty = true;
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::consts;
    use crate::test_helpers::TrayTestContext;

    /// 監視周期パッチが `normalize` で下限へクランプされるようにすること
    #[test]
    fn interval_ms_patch_clamps_below_min() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            interval_ms: Some(10),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_monitor_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.interval_ms),
            consts::MIN_INTERVAL_MS
        );
        assert_eq!(
            ctx.state().monitor_snapshot().interval_ms,
            consts::MIN_INTERVAL_MS
        );
        assert!(config_dirty);
    }

    /// 監視周期パッチが `normalize` で上限へクランプされるようにすること
    #[test]
    fn interval_ms_patch_clamps_above_max() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            interval_ms: Some(999_999),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_monitor_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.interval_ms),
            consts::MAX_INTERVAL_MS
        );
        assert_eq!(
            ctx.state().monitor_snapshot().interval_ms,
            consts::MAX_INTERVAL_MS
        );
        assert!(config_dirty);
    }

    /// イベント監視への切替と同時の周期パッチでは周期をスキップすること
    #[test]
    fn monitor_patch_skips_interval_when_switching_to_event() {
        let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);
        let default_interval = ctx.state().with_config(|c| c.interval_ms);
        let patch = SettingsPatch {
            monitor_mode: Some(MonitorMode::Event),
            interval_ms: Some(500),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_monitor_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.monitor_mode),
            MonitorMode::Event
        );
        assert_eq!(ctx.state().with_config(|c| c.interval_ms), default_interval);
    }

    /// ポーリングへの切替と同時の周期パッチでは周期を反映すること
    #[test]
    fn monitor_patch_applies_interval_when_switching_to_polling() {
        let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Event);
        let patch = SettingsPatch {
            monitor_mode: Some(MonitorMode::Polling),
            interval_ms: Some(500),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_monitor_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.monitor_mode),
            MonitorMode::Polling
        );
        assert_eq!(ctx.state().with_config(|c| c.interval_ms), 500);
        assert!(config_dirty);
        assert!(
            ctx.menu()
                .interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 500 && item.is_checked())
        );
        assert!(!ctx.menu().interval.custom_item.is_checked());
    }

    /// プリセット外の周期パッチでカスタム項目がチェックされること
    #[test]
    fn interval_ms_patch_marks_custom_item() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            interval_ms: Some(750),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_monitor_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(ctx.state().with_config(|c| c.interval_ms), 750);
        assert!(ctx.menu().interval.custom_item.is_checked());
        assert!(ctx.menu().interval.custom_item.text().contains("750"));
        assert!(
            ctx.menu()
                .interval
                .items
                .iter()
                .all(|(item, _)| !item.is_checked())
        );
        assert!(config_dirty);
    }
}

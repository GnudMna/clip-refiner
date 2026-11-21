use std::sync::Arc;

use super::outcome::PatchApplyOutcome;
use crate::tray::clipboard_monitor::bump_monitor_generation_unchecked;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;
use crate::tray::worker::ClipboardWorkerHandle;

// ======================================================================
// 連続記録パッチ
// ======================================================================
/// 連続記録設定のパッチを適用する
pub(super) fn apply_continuous_record_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    _clipboard_worker: &ClipboardWorkerHandle,
    _outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(continuous_record_enabled) = patch.continuous_record_enabled {
        let current = state.with_config(|c| c.continuous_record_enabled);
        if current != continuous_record_enabled {
            menu.continuous_record
                .enabled_item
                .set_checked(continuous_record_enabled);
            state.with_config_mut(|c| c.continuous_record_enabled = continuous_record_enabled);
            *config_dirty = true;
            bump_monitor_generation_unchecked(state);
            if continuous_record_enabled {
                state
                    .clipboard_session()
                    .with_processed_state(|ps| ps.last_written = None);
            }
            if let Err(err) = menu.refresh_continuous_record(state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
            menu.sync_auto_processing_state(state);
        }
    }

    if let Some(continuous_record_limit) = patch.continuous_record_limit {
        state.with_config_mut(|c| {
            c.continuous_record_limit = continuous_record_limit;
            c.normalize();
        });
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
    use crate::security::ContentFingerprint;
    use crate::test_helpers::TrayTestContext;

    /// 連続記録の有効化でメニューと `last_written` を更新すること
    #[test]
    fn continuous_record_enable_updates_menu_and_clears_last_written() {
        let ctx = TrayTestContext::new();
        ctx.state().clipboard_session().with_processed_state(|ps| {
            ps.last_written = Some(ContentFingerprint::from_text("previous"));
        });

        let patch = SettingsPatch {
            continuous_record_enabled: Some(true),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_continuous_record_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

        assert!(ctx.state().with_config(|c| c.continuous_record_enabled));
        assert!(ctx.menu().continuous_record.enabled_item.is_checked());
        assert!(
            ctx.state()
                .clipboard_session()
                .with_processed_state(|ps| ps.last_written.is_none())
        );
        assert!(config_dirty);
    }

    /// 同値の連続記録パッチは no-op となるようにすること
    #[test]
    fn continuous_record_enable_noop_when_unchanged() {
        let ctx = TrayTestContext::new();
        let current = ctx.state().with_config(|c| c.continuous_record_enabled);
        let patch = SettingsPatch {
            continuous_record_enabled: Some(current),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_continuous_record_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

        assert!(!config_dirty);
    }

    /// 連続記録件数パッチが `normalize` でクランプされるようにすること
    #[test]
    fn continuous_record_limit_patch_clamps_to_max() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            continuous_record_limit: Some(999),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_continuous_record_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            ctx.worker(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.continuous_record_limit),
            consts::MAX_CONTINUOUS_RECORD_LIMIT
        );
        assert!(config_dirty);
    }
}

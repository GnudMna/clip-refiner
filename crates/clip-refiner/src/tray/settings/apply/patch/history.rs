use std::sync::Arc;

use super::outcome::PatchApplyOutcome;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::settings::ipc::SettingsPatch;
use crate::tray::state::AppState;

// ======================================================================
// 履歴パッチ
// ======================================================================
/// 履歴設定のパッチを適用する
pub(super) fn apply_history_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(history_enabled) = patch.history_enabled {
        state.with_config_mut(|c| c.history_enabled = history_enabled);
        menu.history.enabled_item.set_checked(history_enabled);
        *config_dirty = true;
        outcome.bump_monitor = true;
        if let Err(err) = menu.refresh_history(state) {
            dispatch::log_menu_operation_error("履歴メニューの更新", err);
        }
    }

    if let Some(keep) = patch.keep_clipboard_exclusion {
        state.with_config_mut(|c| c.keep_clipboard_exclusion = keep);
        *config_dirty = true;
    }

    if let Some(history_limit) = patch.history_limit {
        state.with_config_mut(|c| {
            c.history_limit = history_limit;
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
    use crate::test_helpers::TrayTestContext;

    /// 履歴件数パッチが `normalize` でクランプされるようにすること
    #[test]
    fn history_limit_patch_clamps_to_max() {
        let ctx = TrayTestContext::new();
        let patch = SettingsPatch {
            history_limit: Some(999),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome::default();
        let mut config_dirty = false;

        apply_history_patch(
            &patch,
            ctx.state(),
            ctx.menu(),
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            ctx.state().with_config(|c| c.history_limit),
            consts::MAX_HISTORY_LIMIT
        );
        assert!(config_dirty);
    }
}

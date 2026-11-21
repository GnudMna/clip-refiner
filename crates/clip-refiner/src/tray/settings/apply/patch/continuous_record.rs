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
                state.with_processed_state(|ps| ps.last_written = None);
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

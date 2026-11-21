use std::sync::Arc;

use crate::config::MonitorMode;
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::state::AppState;

// ======================================================================
// 監視設定更新
// ======================================================================
/// 監視モードを更新し、必要に応じて監視用スレッドのリセットを行う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `monitor_mode` - 設定する新しい監視モード
pub fn update_monitor_mode(state: &Arc<AppState>, monitor_mode: MonitorMode) {
    if state.with_config(|c| c.monitor_mode) == monitor_mode {
        return;
    }

    state.with_config_mut(|c| c.monitor_mode = monitor_mode);
    state.save_config();
    bump_monitor_generation(state);
}

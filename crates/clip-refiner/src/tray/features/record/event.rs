use std::sync::Arc;

use crate::tray::clipboard_monitor::bump_monitor_generation_unchecked;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, LockExt};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

/// 連続記録の有効/無効を切り替える
pub(crate) fn toggle_continuous_record(
    menu: &TrayMenu,
    state: &Arc<AppState>,
    _clipboard_worker: &ClipboardWorkerHandle,
) {
    let was_enabled = state.with_config(|c| c.continuous_record_enabled);
    let enabled = !was_enabled;
    menu.continuous_record.enabled_item.set_checked(enabled);
    state.with_config_mut(|c| c.continuous_record_enabled = enabled);
    if enabled {
        // 有効化前の加工書き戻し指紋が、同一内容の再コピーを誤って抑止しないようクリアする
        state
            .clipboard_session()
            .with_processed_state(|ps| ps.last_written = None);
    }
    state.save_config();
    bump_monitor_generation_unchecked(state);
    if let Err(err) = menu.refresh_continuous_record(state) {
        dispatch::log_menu_operation_error("連続記録メニューの更新", err);
    }
    menu.sync_auto_processing_state(state);
}

/// 連続記録に関連するメニューイベント(有効化切替、消去、過去項目の選択)を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_worker` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_continuous_record_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> bool {
    if id == menu.continuous_record.enabled_item.id() {
        toggle_continuous_record(menu, state, clipboard_worker);
        return true;
    }
    if id == menu.continuous_record.clear_item.id() {
        state.clipboard_session().clear_continuous_record();
        state.save_config();
        if let Err(err) = menu.refresh_continuous_record(state) {
            dispatch::log_menu_operation_error("連続記録メニューの更新", err);
        }
        return true;
    }

    let menu_records = menu.continuous_record.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        dispatch::send_clipboard_command(
            clipboard_worker,
            ClipboardCommand::DischargeContinuousRecordAt(*index),
        );
        return true;
    }

    false
}

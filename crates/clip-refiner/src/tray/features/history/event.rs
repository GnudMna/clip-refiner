use std::sync::Arc;

use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, LockExt};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

/// クリップボード履歴に関連するメニューイベント(有効化切替、消去、過去項目の選択)を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_worker` - クリップボードワーカーへの送信ハンドル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_history_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> bool {
    if id == menu.history.enabled_item.id() {
        let enabled = menu.history.enabled_item.is_checked();
        state.with_config_mut(|c| c.history_enabled = enabled);
        state.save_config();
        bump_monitor_generation(state);
        if let Err(err) = menu.refresh_history(state) {
            dispatch::log_menu_operation_error("履歴メニューの更新", err);
        }
        return true;
    }
    if id == menu.history.clear_item.id() {
        state.clipboard_session().clear_history();
        state.save_config();
        if let Err(err) = menu.refresh_history(state) {
            dispatch::log_menu_operation_error("履歴メニューの更新", err);
        }
        return true;
    }

    let menu_records = menu.history.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        if let Some(text) = state.clipboard_session().get_history_entry(*index) {
            dispatch::send_clipboard_command(clipboard_worker, ClipboardCommand::SetText(text));
        }
        return true;
    }

    false
}

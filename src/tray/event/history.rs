use std::sync::Arc;
use std::sync::mpsc::Sender;

use super::super::menu::TrayMenu;
use super::super::state::{AppState, LockExt};
use super::super::worker::ClipboardCommand;

/// クリップボード履歴に関連するメニューイベント(有効化切替、消去、過去項目の選択)を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(super) fn handle_history_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
) -> bool {
    if id == menu.history.enabled_item.id() {
        let enabled = menu.history.enabled_item.is_checked();
        state.with_config_mut(|c| c.history_enabled = enabled);
        state.save_config();
        let _ = menu.refresh_history(state);
        return true;
    }
    if id == menu.history.clear_item.id() {
        state.clear_history();
        state.save_config();
        let _ = menu.refresh_history(state);
        return true;
    }

    let menu_records = menu.history.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        if let Some(text) = state.get_history_entry(*index) {
            let _ = clipboard_tx.send(ClipboardCommand::SetText(text));
        }
        return true;
    }

    false
}

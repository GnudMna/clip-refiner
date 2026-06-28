use std::sync::Arc;
use std::sync::mpsc::Sender;

use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::state::{AppState, LockExt};
use super::super::text_selector::TextSelectorWindow;
use super::super::worker::ClipboardCommand;
use crate::config::AppConfig;
use crate::security::secret_from;

/// 登録文字列メニューイベントを処理する
pub(super) fn handle_texts_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
) -> bool {
    if id == menu.texts.register_item.id() {
        let _ = clipboard_tx.send(ClipboardCommand::RegisterFromClipboard);
        return true;
    }

    let menu_records = menu.texts.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        let text = state.with_config(|config| config.registered_text_at(*index).map(secret_from));
        if let Some(text) = text {
            let _ = clipboard_tx.send(ClipboardCommand::CopyRegisteredText(text));
        }
        return true;
    }

    false
}

/// 登録文字列のクリップボードコピーを実行する
pub(super) fn copy_registered_text(
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    index: usize,
) {
    let Some(text) = state.with_config(|config| config.registered_text_at(index).map(secret_from))
    else {
        return;
    };

    let _ = clipboard_tx.send(ClipboardCommand::CopyRegisteredText(text));
}

/// 登録文字列を削除し、メニューとセレクターを更新する
pub(super) fn delete_registered_text(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    text_selector: &TextSelectorWindow,
    index: usize,
) {
    let removed = state.with_config_mut(|c| c.remove_registered_text(index));
    if !removed {
        return;
    }

    state.save_config();
    refresh_texts_views(state, menu, text_selector);
    notify::show_when_enabled(state, "登録文字列", "登録文字列を削除しました");
}

/// 登録文字列メニューとセレクター表示を設定内容に合わせて更新する
pub(crate) fn refresh_texts_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    text_selector: &TextSelectorWindow,
) {
    let _ = menu.refresh_texts(state);
    if text_selector.is_visible() {
        let texts_json = state.with_config(AppConfig::texts_to_json_list);
        text_selector.refresh_items(&texts_json);
    }
}

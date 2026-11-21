use std::sync::Arc;
use std::sync::mpsc::Sender;

use super::super::clip_selector::ClipSelectorWindow;
use super::super::dispatch;
use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::state::{AppState, LockExt};
use super::super::worker::ClipboardCommand;

use crate::config::AppConfig;

// ======================================================================
// メニューイベント処理
// ======================================================================
/// 登録クリップメニューイベントを処理する
pub(super) fn handle_clips_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    _state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
) -> bool {
    if id == menu.clips.register_item.id() {
        dispatch::send_clipboard_command(clipboard_tx, ClipboardCommand::RegisterClipFromClipboard);
        return true;
    }

    let menu_records = menu.clips.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        dispatch::send_clipboard_command(
            clipboard_tx,
            ClipboardCommand::CopyRegisteredClip(*index),
        );
        return true;
    }

    false
}

// ======================================================================
// 登録クリップ操作
// ======================================================================
/// 登録クリップのクリップボードコピーを実行する
pub(super) fn copy_registered_clip(
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    index: usize,
) {
    let exists = state.with_config(|config| config.clips.get(index).is_some());
    if !exists {
        return;
    }

    dispatch::send_clipboard_command(clipboard_tx, ClipboardCommand::CopyRegisteredClip(index));
}

/// 登録クリップを削除し、メニューとセレクターを更新する
pub(super) fn delete_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
) {
    let removed = state.with_config_mut(|c| c.remove_registered_clip(index));
    if !removed {
        return;
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    notify::show_when_enabled(state, "登録クリップ", "登録クリップを削除しました");
}

// ======================================================================
// UI 更新
// ======================================================================
/// 登録クリップメニューとセレクター表示を設定内容に合わせて更新する
pub(crate) fn refresh_clips_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
) {
    if let Err(err) = menu.refresh_clips(state) {
        dispatch::log_menu_operation_error("登録クリップメニューの更新", err);
    }
    if clip_selector.is_visible() {
        let clips_json = state.with_config(AppConfig::clips_to_json_list);
        clip_selector.refresh_items(&clips_json);
    }
}

use std::sync::Arc;
use std::sync::mpsc::Sender;

use super::super::menu::TrayMenu;
use super::super::state::AppState;
use super::super::worker::ClipboardCommand;
use crate::refiner::RefineMode;
use crate::tray::state::LockExt;

/// 加工モードを更新し、メニューの状態や設定ファイルへ反映させる
///
/// 必要に応じてクリップボードワーカーに加工命令を送信する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
/// * `mode` - 設定する新しい加工モード
pub fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard_tx: &Sender<ClipboardCommand>,
    mode: RefineMode,
) {
    state.with_config_mut(|c| c.mode = mode);

    menu.refine
        .favorite_records
        .lock_ignore_poison()
        .iter()
        .chain(menu.refine.all_mode_items())
        .for_each(|(item, m)| item.set_checked(*m == mode));
    menu.refresh_category_labels(mode);

    let favorite_modes = state.with_config(|config| config.favorite_modes.clone());
    menu.refine.sync_favorite_actions(mode, &favorite_modes);

    state.save_config();
    let _ = clipboard_tx.send(ClipboardCommand::ProcessMode(mode));
}

/// 加工モードの選択メニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(super) fn handle_refine_mode_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
) -> bool {
    if let Some((_, mode)) = menu
        .refine
        .favorite_records
        .lock_ignore_poison()
        .iter()
        .find(|(item, _)| item.id() == id)
    {
        update_refine(state, menu, clipboard_tx, *mode);
        return true;
    }

    if let Some((_, mode)) = menu
        .refine
        .all_mode_items()
        .find(|(item, _)| item.id() == id)
    {
        update_refine(state, menu, clipboard_tx, *mode);
        true
    } else {
        false
    }
}

use std::sync::Arc;

use super::favorites::refresh_quick_selector_modes;
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::state::AppState;
use crate::tray::state::LockExt;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// 変換モード更新
// ======================================================================
/// 変換モードを更新し、メニューの状態や設定ファイルへ反映させる
///
/// 必要に応じてクリップボードワーカーに加工命令を送信する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
/// * `mode` - 設定する新しい変換モード
pub fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard_worker: &ClipboardWorkerHandle,
    mode: RefineMode,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    state.with_config_mut(|c| {
        c.mode = mode;
        c.pipeline.clear();
    });

    let active_modes: std::collections::HashSet<RefineMode> = std::iter::once(mode).collect();

    menu.refine
        .favorite_records
        .lock_ignore_poison()
        .iter()
        .chain(menu.refine.all_mode_items())
        .for_each(|(item, m)| item.set_checked(active_modes.contains(m)));
    menu.refresh_category_labels(mode);

    let favorite_modes = state.with_config(|config| config.favorite_modes.clone());
    menu.refine.sync_favorite_actions(mode, &favorite_modes);

    state.save_config();
    dispatch::send_clipboard_command(clipboard_worker, ClipboardCommand::ProcessMode(mode));
    refresh_quick_selector_modes(state, quick_selector);
}

// ======================================================================
// メニューイベント処理
// ======================================================================
/// 変換モードの選択メニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_refine_mode_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
    quick_selector: Option<&QuickSelectorWindow>,
) -> bool {
    if let Some(mode) = menu
        .refine
        .favorite_records
        .lock_ignore_poison()
        .iter()
        .find(|(item, _)| item.id() == id)
        .map(|(_, mode)| *mode)
    {
        update_refine(state, menu, clipboard_worker, mode, quick_selector);
        return true;
    }

    if let Some((_, mode)) = menu
        .refine
        .all_mode_items()
        .find(|(item, _)| item.id() == id)
    {
        update_refine(state, menu, clipboard_worker, *mode, quick_selector);
        true
    } else {
        false
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use crate::refiner::RefineMode;
    use crate::test_helpers::TrayTestContext;
    use crate::tray::state::LockExt;

    /// お気に入りサブメニューの `MenuId` からモードを解決できるようにすること
    #[test]
    fn resolve_favorite_mode_from_menu_id() {
        let ctx = TrayTestContext::with_config(|config| {
            config.favorite_modes = vec![RefineMode::Trim];
        });
        let item_id = ctx
            .menu()
            .refine
            .favorite_records
            .lock_ignore_poison()
            .first()
            .expect("お気に入り項目が存在する")
            .0
            .id()
            .clone();

        let resolved = ctx
            .menu()
            .refine
            .favorite_records
            .lock_ignore_poison()
            .iter()
            .find(|(item, _)| item.id() == &item_id)
            .map(|(_, mode)| *mode);

        assert_eq!(resolved, Some(RefineMode::Trim));
    }
}

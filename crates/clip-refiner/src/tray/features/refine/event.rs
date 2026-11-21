use std::sync::Arc;

use super::favorites::refresh_quick_selector_modes;
use crate::refiner::RefineMode;
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::notify::{NotificationCategory, show_category_notification};
use crate::tray::selector::QuickSelectorWindow;
use crate::tray::state::LockExt;
use crate::tray::state::{AppState, SettingsEvent};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// 変換モード更新
// ======================================================================
/// 自動変換の変換モードを更新し、メニューの状態や設定ファイルへ反映させる
///
/// パイプラインは解除し、複数段だった場合は通知で知らせる。
/// 現在のクリップボードにも新しいモードを適用する。
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
    let cleared_pipeline = state.with_config_mut(|c| {
        c.mode = mode;
        std::mem::take(&mut c.pipeline)
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
    menu.sync_auto_processing_state(state);

    state.save_config();
    // 黙ってパイプラインが消えると監視結果が変わった理由が分からないため明示する
    if cleared_pipeline.len() > 1 {
        show_category_notification(
            state,
            NotificationCategory::Process,
            "自動変換を変更",
            &format!(
                "パイプライン ({}) を解除し、{} に切り替えました",
                RefineMode::pipeline_label(&cleared_pipeline),
                mode.label()
            ),
        );
    }
    dispatch::send_clipboard_command(clipboard_worker, ClipboardCommand::ProcessMode(mode));
    refresh_quick_selector_modes(state, quick_selector);
}

/// 変換モードを自動変換に設定し、自動変換が無効なら有効にする
///
/// トレイの変換モードメニューやクイックセレクターの `Ctrl+Enter` など、
/// 利用者が自動変換のモードとして明示的に選んだときに使う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `clipboard_worker` - クリップボード・ワーカーへの送信ハンドル
/// * `mode` - 設定する変換モード
/// * `quick_selector` - 一覧を再描画するクイックセレクター
pub(crate) fn select_auto_refine_mode(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard_worker: &ClipboardWorkerHandle,
    mode: RefineMode,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    if set_auto_refine_enabled(state, menu, true) {
        let _ = state.proxy.send_event(SettingsEvent::Refresh.into());
    }
    update_refine(state, menu, clipboard_worker, mode, quick_selector);
}

/// コピー時の自動変換の有効・無効を切り替え、メニュー表示と監視状態へ反映する
///
/// 設定ファイルへの保存は呼び出し側で行う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `enabled` - 自動変換を有効にする場合は `true`
///
/// # Returns
/// * `bool` - 設定値が変わった場合は `true`
pub(crate) fn set_auto_refine_enabled(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    enabled: bool,
) -> bool {
    let changed = state.with_config_mut(|c| {
        let changed = c.auto_refine_enabled != enabled;
        c.auto_refine_enabled = enabled;
        changed
    });
    menu.sync_auto_processing_state(state);
    if changed {
        // 切り替え時点のクリップボードは加工せず、次のコピーから対象にする
        bump_monitor_generation(state);
    }
    changed
}

/// 自動変換の設定を変えずに、現在のクリップボードへ変換モードを一回だけ適用する
///
/// # Arguments
/// * `clipboard_worker` - クリップボード・ワーカーへの送信ハンドル
/// * `mode` - 適用する変換モード
pub(crate) fn apply_refine_once(clipboard_worker: &ClipboardWorkerHandle, mode: RefineMode) {
    dispatch::send_clipboard_command(clipboard_worker, ClipboardCommand::ProcessMode(mode));
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
    if id == menu.refine.auto_refine_item.id() {
        let enabled = menu.refine.auto_refine_item.is_checked();
        if set_auto_refine_enabled(state, menu, enabled) {
            state.save_config();
            let _ = state.proxy.send_event(SettingsEvent::Refresh.into());
        }
        return true;
    }

    if let Some(mode) = menu
        .refine
        .favorite_records
        .lock_ignore_poison()
        .iter()
        .find(|(item, _)| item.id() == id)
        .map(|(_, mode)| *mode)
    {
        select_auto_refine_mode(state, menu, clipboard_worker, mode, quick_selector);
        return true;
    }

    if let Some((_, mode)) = menu
        .refine
        .all_mode_items()
        .find(|(item, _)| item.id() == id)
    {
        select_auto_refine_mode(state, menu, clipboard_worker, *mode, quick_selector);
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

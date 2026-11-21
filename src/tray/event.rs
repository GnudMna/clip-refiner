use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::menu::TrayMenu;
use super::monitor::spawn_monitor_thread;
use super::notifier;
use super::state::{AppState, LockExt};
use super::worker::ClipboardCommand;
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::RefineMode;

use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

// ======================================================================
// メニューイベント処理
// ======================================================================
/// システムトレイアイコンのメニューから受信したイベントを処理する
///
/// クリックされたメニュー項目の ID に基づいて、アプリケーション設定の変更、
/// 履歴操作、加工モードの切り替え、またはプログラムの終了などを実行します。
///
/// # Arguments
/// * `event` - 受信したメニューイベント
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
/// * `control_flow` - イベントループの制御フロー
pub fn handle_menu_event(
    event: MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
    control_flow: &mut ControlFlow,
) {
    if handle_app_control(&event.id, menu, state, control_flow) {
        return;
    }
    if handle_history_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    if handle_notification_event(&event.id, menu, state) {
        return;
    }
    if handle_refine_mode_event(&event.id, menu, state, clipboard_tx) {
        return;
    }
    handle_monitor_event(&event.id, menu, state);
}

/// UIウィンドウ（セレクタ）に関連するイベントを処理する
///
/// 主にフォーカス喪失時の自動非表示処理などを行います。
///
/// # Arguments
/// * `event` - 受信したウィンドウイベント
/// * `selector` - セレクタウィンドウのインスタンス
/// * `last_selector_show` - セレクタが最後に表示された時刻
pub fn handle_window_event(
    event: WindowEvent,
    selector: &super::selector::SelectorWindow,
    last_selector_show: &Instant,
) {
    if let WindowEvent::Focused(focused) = event
        && !focused
        && selector.is_visible()
    {
        // 表示直後のフォーカスロスト（WindowsのAltキーイベント等によるもの）を無視する
        if last_selector_show.elapsed().as_millis() > 200 {
            selector.hide();
        }
    }
}

// ======================================================================
// アプリケーション制御
// ======================================================================
/// アプリケーションの基本操作（終了、一時停止、ショートカット一覧表示）を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `control_flow` - イベントループの制御フロー
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返します。
fn handle_app_control(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    control_flow: &mut ControlFlow,
) -> bool {
    if id == menu.quit_item.id() {
        *control_flow = ControlFlow::Exit;
        true
    } else if id == menu.pause_item.id() {
        let paused = menu.pause_item.is_checked();
        state.with_config_mut(|c| c.is_paused = paused);
        notifier::show_pause_notification(state, paused, "設定変更");
        state.save_config();
        spawn_monitor_thread(Arc::clone(state));
        true
    } else if id == menu.shortcut_list_item.id() {
        notification::show_notification(
            "ショートカット一覧",
            "Alt + Shift + S: クイックセレクター\nAlt + Shift + N: 成功通知の切替\nAlt + Shift + P: 一時停止/再開\nAlt + Shift + Q: 終了",
        );
        true
    } else {
        false
    }
}

// ======================================================================
// 履歴
// ======================================================================
/// クリップボード履歴に関連するメニューイベント（有効化切替、消去、過去項目の選択）を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返します。
fn handle_history_event(
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

    if let Some((_, text)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        let _ = clipboard_tx.send(ClipboardCommand::SetText(text.clone()));
        return true;
    }

    false
}

// ======================================================================
// 通知設定
// ======================================================================
/// 通知設定に関連するメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返します。
fn handle_notification_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if id == menu.notification.enabled_item.id() {
        let enabled = menu.notification.enabled_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.enabled = enabled);
        menu.notification.content_submenu.set_enabled(enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_mode_item.id() {
        let enabled = menu.notification.notify_mode_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_mode = enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_result_item.id() {
        let enabled = menu.notification.notify_result_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_result = enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_pause_item.id() {
        let enabled = menu.notification.notify_pause_item.is_checked();
        state.with_config_mut(|c| c.notification_settings.notify_pause = enabled);
        state.save_config();
        return true;
    }
    false
}

// ======================================================================
// 加工モード
// ======================================================================
/// 加工モードの選択メニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_tx` - クリップボード・ワーカーへの送信チャネル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返します。
fn handle_refine_mode_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_tx: &Sender<ClipboardCommand>,
) -> bool {
    if let Some((_, mode)) = menu.refine.all_items().find(|(item, _)| item.id() == id) {
        update_refine(state, menu, clipboard_tx, *mode);
        true
    } else {
        false
    }
}

// ======================================================================
// 監視設定
// ======================================================================
/// 監視設定（監視モード、ポーリング間隔）に関連するメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返します。
fn handle_monitor_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if let Some((_, monitor_mode)) = menu.monitor.items.iter().find(|(item, _)| item.id() == id) {
        update_monitor_mode(state, menu, *monitor_mode);
        return true;
    }

    for (item, ms) in &menu.interval.items {
        if item.id() == id {
            state.with_config_mut(|c| c.interval_ms = *ms);
            for (it, _) in &menu.interval.items {
                it.set_checked(false);
            }
            item.set_checked(true);
            state.save_config();
            return true;
        }
    }
    false
}

// ======================================================================
// モード・監視更新
// ======================================================================
/// 加工モードを更新し、メニューの状態や設定ファイルへ反映させる
///
/// 必要に応じてクリップボードワーカーに加工命令を送信します。
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
        .all_items()
        .for_each(|(item, m)| item.set_checked(*m == mode));
    menu.refresh_category_labels(mode);

    state.save_config();
    let _ = clipboard_tx.send(ClipboardCommand::ProcessMode(mode));
}

/// 監視モードを更新し、必要に応じて監視用スレッドのリセットを行う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `monitor_mode` - 設定する新しい監視モード
pub fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    if state.with_config(|c| c.monitor_mode) == monitor_mode {
        return;
    }

    state.with_config_mut(|c| c.monitor_mode = monitor_mode);

    for (item, m) in &menu.monitor.items {
        item.set_checked(*m == monitor_mode);
    }

    super::monitor::update_monitor_mode_impl(menu, monitor_mode);

    state.save_config();
    super::monitor::spawn_monitor_thread(Arc::clone(state));
}

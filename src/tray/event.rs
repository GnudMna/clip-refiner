use std::sync::{Arc, atomic::Ordering};

use super::menu::TrayMenu;
use super::monitor::spawn_monitor_thread;
use super::notifier;
use super::state::{AppState, LockExt};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use arboard::Clipboard;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

/// トレイアイコンメニューから受信したイベントを処理する。
///
/// # Arguments
/// * `event` - 受信したメニューイベント。
/// * `menu` - トレイメニュー構造体。
/// * `state` - アプリケーションの状態。
/// * `clipboard` - クリップボード・ハンドラ。
/// * `control_flow` - イベントループの制御フロー。
pub fn handle_menu_event(
    event: MenuEvent,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard: &mut Clipboard,
    control_flow: &mut ControlFlow,
) {
    if handle_app_control(&event.id, menu, state, control_flow) {
        return;
    }
    if handle_history_event(&event.id, menu, state, clipboard) {
        return;
    }
    if handle_notification_event(&event.id, menu, state) {
        return;
    }
    if handle_refine_mode_event(&event.id, menu, state, clipboard) {
        return;
    }
    handle_monitor_event(&event.id, menu, state);
}

/// ウィンドウイベント（フォーカスロストなど）を処理する。
///
/// # Arguments
/// * `event` - 受信したウィンドウイベント。
/// * `selector` - セレクターウィンドウ。
/// * `last_selector_show` - セレクターが最後に表示された時刻。
pub fn handle_window_event(
    event: WindowEvent,
    selector: &super::selector::SelectorWindow,
    last_selector_show: &std::time::Instant,
) {
    if let WindowEvent::Focused(focused) = event {
        if !focused && selector.is_visible() {
            // 表示直後のフォーカスロスト（WindowsのAltキーイベント等によるもの）を無視する
            if last_selector_show.elapsed().as_millis() > 200 {
                selector.hide();
            }
        }
    }
}

/// アプリケーションの基本操作（終了、一時停止、ショートカット一覧）を処理する。
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID。
/// * `menu` - トレイメニュー。
/// * `state` - アプリケーションの状態。
/// * `control_flow` - イベントループの制御フロー。
///
/// # Returns
/// * `bool` - イベントがこの関数で処理された場合は `true`、それ以外は `false`。
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
        state.set_paused(paused);
        notifier::show_pause_notification(state, paused, "設定変更");
        state.save_config();
        if !paused {
            spawn_monitor_thread(Arc::clone(state));
        }
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

/// クリップボード履歴に関連するイベントを処理する。
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID。
/// * `menu` - トレイメニュー。
/// * `state` - アプリケーションの状態。
/// * `clipboard` - クリップボード・ハンドラ。
///
/// # Returns
/// * `bool` - イベントがこの関数で処理された場合は `true`、それ以外は `false`。
fn handle_history_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard: &mut Clipboard,
) -> bool {
    if id == menu.history.enabled_item.id() {
        let enabled = menu.history.enabled_item.is_checked();
        state.set_history_enabled(enabled);
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
        if let Err(e) = clipboard.set_text(text.clone()) {
            notification::show_anyhow_error("クリップボード設定エラー", &anyhow::anyhow!(e));
        } else {
            state.set_last_processed_text(text.clone());
            if state.show_success_notification() {
                notification::show_notification("履歴から復元", "クリップボードにコピーしました");
            }
        }
        return true;
    }

    false
}

/// 通知設定に関連するイベントを処理する。
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID。
/// * `menu` - トレイメニュー。
/// * `state` - アプリケーションの状態。
///
/// # Returns
/// * `bool` - イベントがこの関数で処理された場合は `true`、それ以外は `false`。
fn handle_notification_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if id == menu.notification.enabled_item.id() {
        let enabled = menu.notification.enabled_item.is_checked();
        state.set_show_success_notification(enabled);
        menu.notification.content_submenu.set_enabled(enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_mode_item.id() {
        let enabled = menu.notification.notify_mode_item.is_checked();
        state.set_notify_mode(enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_result_item.id() {
        let enabled = menu.notification.notify_result_item.is_checked();
        state.set_notify_result(enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_pause_item.id() {
        let enabled = menu.notification.notify_pause_item.is_checked();
        state.set_notify_pause(enabled);
        state.save_config();
        return true;
    }
    false
}

/// 加工モードの選択イベントを処理する。
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID。
/// * `menu` - トレイメニュー。
/// * `state` - アプリケーションの状態。
/// * `clipboard` - クリップボード・ハンドラ。
///
/// # Returns
/// * `bool` - イベントがこの関数で処理された場合は `true`、それ以外は `false`。
fn handle_refine_mode_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard: &mut Clipboard,
) -> bool {
    if let Some((_, mode)) = menu.refine.all_items().find(|(item, _)| item.id() == id) {
        update_refine(state, menu, clipboard, *mode);
        true
    } else {
        false
    }
}

/// 監視設定（監視モード、ポーリング間隔）に関連するイベントを処理する。
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID。
/// * `menu` - トレイメニュー。
/// * `state` - アプリケーションの状態。
///
/// # Returns
/// * `bool` - イベントがこの関数で処理された場合は `true`、それ以外は `false`。
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
            state.set_interval_ms(*ms);
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

/// 選択された加工モードをアプリケーションの状態に反映し、UIを更新する。
///
/// # Arguments
/// * `state` - アプリケーションの状態。
/// * `menu` - トレイメニュー構造体。
/// * `clipboard` - クリップボード・ハンドラ。
/// * `mode` - 設定する加工モード。
pub fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard: &mut Clipboard,
    mode: RefineMode,
) {
    state.set_mode(mode);

    menu.refine
        .all_items()
        .for_each(|(item, m)| item.set_checked(*m == mode));
    menu.refresh_category_labels(mode);

    state.save_config();
    if let Some(processed) = process_clipboard(clipboard, mode) {
        state.set_last_processed_text(processed.clone());
        notifier::show_process_notification(state, mode, &processed);
    }
}

/// 監視モードを更新し、それに応じたスレッドを再起動する。
///
/// # Arguments
/// * `state` - アプリケーションの状態。
/// * `menu` - トレイメニュー構造体。
/// * `monitor_mode` - 設定する監視モード。
pub fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    if state.get_monitor_mode() == monitor_mode {
        return;
    }

    state.set_monitor_mode(monitor_mode);

    for (item, m) in &menu.monitor.items {
        item.set_checked(*m == monitor_mode);
    }

    super::monitor::update_monitor_mode_impl(menu, monitor_mode);

    state.save_config();
    super::monitor::spawn_monitor_thread(Arc::clone(state));
}

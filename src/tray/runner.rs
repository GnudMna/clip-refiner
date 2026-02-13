use std::sync::{Arc, atomic::Ordering};

use super::menu::TrayMenu;
use super::monitor::{init_clipboard, spawn_monitor_thread, update_monitor_mode_impl};
use super::notifier;
use super::selector::init_selector;
use super::state::{AppEvent, AppState, LockExt};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use anyhow::Result;
use arboard::Clipboard;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
#[cfg(windows)]
use tao::platform::windows::EventLoopBuilderExtWindows;
use tray_icon::menu::MenuEvent;

/// アプリケーションのメインループを開始する。
///
/// この関数はイベントループを初期化し、トレイアイコンとメニューを設定する。
/// クリップボード監視用の別スレッドを起動し、メニューからのイベントを待ち受ける。
/// イベントループはアプリケーションが終了するまでブロックされる。
pub fn run_loop() -> Result<()> {
    let event_loop = create_event_loop();
    let proxy = event_loop.create_proxy();
    let state = Arc::new(AppState::new(proxy.clone()));
    let menu = TrayMenu::build(&state)?;

    // グローバルショートカットの初期化
    let hotkey_manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
    let selector_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);
    let notification_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyN);
    let pause_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyP);
    let quit_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyQ);

    let register = |hotkey| {
        hotkey_manager
            .register(hotkey)
            .map_err(|e| anyhow::anyhow!(e))
    };
    register(selector_hotkey)?;
    register(notification_hotkey)?;
    register(pause_hotkey)?;
    register(quit_hotkey)?;

    // ホットキーイベントをイベントループに転送するスレッドを開始
    let hotkey_proxy = proxy.clone();
    std::thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.recv() {
            let _ = hotkey_proxy.send_event(AppEvent::Hotkey(event));
        }
    });

    // クイックセレクターの初期化
    let selector = init_selector(&event_loop, proxy.clone())?;

    // 初期状態で履歴メニューを更新
    menu.refresh_history(&state)?;

    // クリップボード監視スレッドの開始
    let state_for_monitor = Arc::clone(&state);
    spawn_monitor_thread(state_for_monitor);

    let menu_channel = MenuEvent::receiver();
    let mut clipboard = init_clipboard()?;
    let mut last_selector_show = std::time::Instant::now();

    // イベントループの実行
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            tao::event::Event::UserEvent(AppEvent::RequestModeChange(mode)) => {
                selector.hide();
                update_refine(&state, &menu, &mut clipboard, mode);
            }
            tao::event::Event::UserEvent(AppEvent::HideSelector) => {
                selector.hide();
            }
            tao::event::Event::UserEvent(AppEvent::RefreshHistory) => {
                let _ = menu.refresh_history(&state);
            }
            tao::event::Event::UserEvent(AppEvent::Hotkey(event)) => {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    if event.id == selector_hotkey.id() {
                        if selector.is_visible() {
                            selector.hide();
                        } else {
                            last_selector_show = std::time::Instant::now();
                            selector.show(state.get_mode());
                        }
                    } else if event.id == notification_hotkey.id() {
                        let new_val = !state.show_success_notification.load(Ordering::Relaxed);
                        state
                            .show_success_notification
                            .store(new_val, Ordering::Relaxed);
                        menu.notification.enabled_item.set_checked(new_val);
                        menu.notification.content_submenu.set_enabled(new_val);
                        state.save_config();
                        notification::show_notification(
                            "ショートカット",
                            if new_val {
                                "成功通知を有効にしました"
                            } else {
                                "成功通知を無効にしました"
                            },
                        );
                    } else if event.id == pause_hotkey.id() {
                        let new_paused = !state.paused.load(Ordering::Relaxed);
                        state.paused.store(new_paused, Ordering::Relaxed);
                        menu.pause_item.set_checked(new_paused);
                        notifier::show_pause_notification(&state, new_paused, "ショートカット");
                        if !new_paused {
                            spawn_monitor_thread(Arc::clone(&state));
                        }
                    } else if event.id == quit_hotkey.id() {
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            tao::event::Event::WindowEvent {
                window_id, event, ..
            } => match event {
                tao::event::WindowEvent::Focused(focused) => {
                    if !focused && window_id == selector.id() && selector.is_visible() {
                        // 表示直後のフォーカスロスト（WindowsのAltキーイベント等によるもの）を無視する
                        if last_selector_show.elapsed().as_millis() > 200 {
                            selector.hide();
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if let Ok(event) = menu_channel.try_recv() {
            handle_menu_event(event, &menu, &state, &mut clipboard, control_flow);
        }
    })
}

/// プラットフォームに応じたイベントループを作成する
///
/// Windows環境では `with_any_thread(true)` を設定し、
/// メインスレッド以外でもイベントループに関連する操作を行えるようにする。
fn create_event_loop() -> EventLoop<AppEvent> {
    #[cfg(windows)]
    return EventLoopBuilder::<AppEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    #[cfg(not(windows))]
    return EventLoopBuilder::<AppEvent>::with_user_event().build();
}

/// トレイアイコンメニューから受信したイベントを処理する。
///
/// 各メニュー項目（終了、一時停止、モード変更など）に対応するアクションを実行する。
///
/// # Arguments
/// * `event` - 受信したメニューイベント。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
/// * `clipboard` - クリップボードのインスタンス。
/// * `control_flow` - イベントループの制御フラグ。
fn handle_menu_event(
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

/// アプリケーション制御イベント（終了、一時停止、ショートカット一覧）を処理する。
///
/// # Arguments
/// * `id` - メニュー項目のID。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
/// * `control_flow` - イベントループの制御フラグ。
///
/// # Returns
/// * `bool` - イベントが処理された場合は `true`、そうでない場合は `false`。
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
        state.paused.store(paused, Ordering::Relaxed);
        notifier::show_pause_notification(state, paused, "設定変更");
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

/// 履歴関連のイベント（有効化、クリア、履歴からの復元）を処理する。
///
/// # Arguments
/// * `id` - メニュー項目のID。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
/// * `clipboard` - クリップボードのインスタンス。
///
/// # Returns
/// * `bool` - イベントが処理された場合は `true`、そうでない場合は `false`。
fn handle_history_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard: &mut Clipboard,
) -> bool {
    if id == menu.history.enabled_item.id() {
        let enabled = menu.history.enabled_item.is_checked();
        state.history_enabled.store(enabled, Ordering::Relaxed);
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

    // 履歴アイテムのクリック判定
    // メニューIDと一致する履歴を探す
    let menu_records = menu.history.records.lock_ignore_poison();

    if let Some((_, text)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        if let Err(e) = clipboard.set_text(text.clone()) {
            notification::show_anyhow_error("クリップボード設定エラー", &anyhow::anyhow!(e));
        } else {
            state.set_last_processed_text(text.clone());
            if state.show_success_notification.load(Ordering::Relaxed) {
                notification::show_notification("履歴から復元", "クリップボードにコピーしました");
            }
        }
        return true;
    }

    false
}

/// 通知設定関連のイベントを処理する。
///
/// # Arguments
/// * `id` - メニュー項目のID。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
///
/// # Returns
/// * `bool` - イベントが処理された場合は `true`、そうでない場合は `false`。
fn handle_notification_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if id == menu.notification.enabled_item.id() {
        let enabled = menu.notification.enabled_item.is_checked();
        state
            .show_success_notification
            .store(enabled, Ordering::Relaxed);
        menu.notification.content_submenu.set_enabled(enabled);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_mode_item.id() {
        let enabled = menu.notification.notify_mode_item.is_checked();
        state
            .notification_notify_mode
            .store(enabled, Ordering::Relaxed);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_result_item.id() {
        let enabled = menu.notification.notify_result_item.is_checked();
        state
            .notification_notify_result
            .store(enabled, Ordering::Relaxed);
        state.save_config();
        return true;
    }
    if id == menu.notification.notify_pause_item.id() {
        let enabled = menu.notification.notify_pause_item.is_checked();
        state
            .notification_notify_pause
            .store(enabled, Ordering::Relaxed);
        state.save_config();
        return true;
    }
    false
}

/// 加工モード選択イベントを処理する。
///
/// # Arguments
/// * `id` - メニュー項目のID。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
/// * `clipboard` - クリップボードのインスタンス。
///
/// # Returns
/// * `bool` - イベントが処理された場合は `true`、そうでない場合は `false`。
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

/// 監視設定（監視方式、監視間隔）イベントを処理する。
///
/// # Arguments
/// * `id` - メニュー項目のID。
/// * `menu` - トレイメニューのインスタンス。
/// * `state` - アプリケーションの共有状態。
///
/// # Returns
/// * `bool` - イベントが処理された場合は `true`、そうでない場合は `false`。
fn handle_monitor_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    if let Some((_, monitor_mode)) = menu.monitor.items.iter().find(|(item, _)| item.id() == id) {
        update_monitor_mode(state, menu, *monitor_mode);
        return true;
    }

    // 監視周期アイテム（ミリ秒）から該当するものを検索
    for (item, ms) in &menu.interval.items {
        if item.id() == id {
            state.interval_ms.store(*ms, Ordering::Relaxed);
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
/// 新しいモードを状態に保存し、すべてのモード選択メニューのチェック状態を更新する。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `menu` - トレイメニューのインスタンス。
/// * `clipboard` - クリップボードのインスタンス。
/// * `mode` - 新しく選択された加工モード。
fn update_refine(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard: &mut Clipboard,
    mode: RefineMode,
) {
    state.set_mode(mode);

    // すべてのモードアイテムをイテレートして、選択されたモードのチェック状態を更新
    menu.refine
        .all_items()
        .for_each(|(item, m)| item.set_checked(*m == mode));
    menu.refresh_category_labels(mode);

    state.save_config();
    if let Some(processed) = process_clipboard(clipboard, mode) {
        state.set_last_processed_text(processed.clone());
        notifier::show_process_notification(&state, mode, &processed);
    }
}

/// 監視方式（ポーリング/イベント）を切り替える。
///
/// 新しい監視方式を状態に保存し、メニューのチェック状態を更新する。
/// 方式の変更に応じて、監視周期メニューの有効/無効を切り替える。
/// 最後に、新しい方式で動作する監視スレッドを再起動する。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `menu` - トレイメニューのインスタンス。
/// * `monitor_mode` - 新しく選択された監視方式。
fn update_monitor_mode(state: &Arc<AppState>, menu: &TrayMenu, monitor_mode: MonitorMode) {
    // モードが変わっていない場合は何もしない
    if state.get_monitor_mode() == monitor_mode {
        return;
    }

    // 監視モードを更新
    state.set_monitor_mode(monitor_mode);

    // メニューのチェック状態を更新
    for (item, m) in &menu.monitor.items {
        item.set_checked(*m == monitor_mode);
    }

    // 監視周期メニューの有効/無効を切り替え
    update_monitor_mode_impl(menu, monitor_mode);

    state.save_config();

    // 監視スレッドを再起動（世代を更新することで旧スレッドを終了させる）
    spawn_monitor_thread(Arc::clone(state));
}

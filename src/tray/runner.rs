use std::sync::{Arc, atomic::Ordering};

use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use super::menu::TrayMenu;
use super::monitor::{
    init_clipboard, show_process_notification, spawn_monitor_thread, update_monitor_mode_impl,
};
use super::state::{AppEvent, AppState, LockExt};

use anyhow::Result;
use arboard::Clipboard;
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

    // 初期状態で履歴メニューを更新
    menu.refresh_history(&state)?;

    // クリップボード監視スレッドの開始
    let state_for_monitor = Arc::clone(&state);
    spawn_monitor_thread(state_for_monitor);

    let menu_channel = MenuEvent::receiver();
    let mut clipboard = init_clipboard()?;

    // イベントループの実行
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            tao::event::Event::UserEvent(AppEvent::RefreshHistory) => {
                let _ = menu.refresh_history(&state);
            }
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
    if event.id == menu.quit_item.id() {
        *control_flow = ControlFlow::Exit;
    } else if event.id == menu.pause_item.id() {
        state
            .paused
            .store(menu.pause_item.is_checked(), Ordering::Relaxed);
    } else if event.id == menu.history_enabled_item.id() {
        let enabled = menu.history_enabled_item.is_checked();
        state.history_enabled.store(enabled, Ordering::Relaxed);
        state.save_config();
        let _ = menu.refresh_history(state);
    } else if event.id == menu.clear_history_item.id() {
        state.clear_history();
        state.save_config();
        let _ = menu.refresh_history(state);
    } else if let Some((_, text)) = menu
        .history_records
        .lock_ignore_poison()
        .iter()
        .find(|(id, _)| event.id == *id)
    {
        match Clipboard::new() {
            Ok(mut cb) => {
                if let Err(e) = cb.set_text(text.clone()) {
                    notification::error::show_anyhow_error(
                        "クリップボード設定エラー",
                        &anyhow::anyhow!(e),
                    );
                } else {
                    state.set_last_processed_text(text.clone());
                    notification::success::show_success_debug_notification(
                        "履歴から復元",
                        "クリップボードにコピーしました",
                    );
                }
            }
            Err(e) => notification::error::show_anyhow_error(
                "クリップボード初期化エラー",
                &anyhow::anyhow!(e),
            ),
        }
    } else if let Some((_, mode)) = menu
        .mode_items // 全てのモード関連アイテムをチェーンして検索
        .iter()
        .chain(menu.line_actions_items.iter())
        .chain(menu.trim_items.iter())
        .chain(menu.escape_items.iter())
        .chain(menu.json_format_items.iter())
        .chain(menu.json_to_yaml_items.iter())
        .chain(menu.yaml_to_json_items.iter())
        .chain(menu.datetime_items.iter())
        .chain(menu.number_items.iter())
        .find(|(item, _)| event.id == item.id())
    {
        update_refine(state, menu, clipboard, *mode);
    } else if let Some((_, monitor_mode)) = menu
        .monitor_mode_items
        .iter()
        .find(|(item, _)| event.id == item.id())
    {
        update_monitor_mode(state, menu, *monitor_mode);
    } else {
        for (item, ms) in &menu.interval_items {
            if event.id == item.id() {
                state.interval_ms.store(*ms, Ordering::Relaxed);
                for (it, _) in &menu.interval_items {
                    it.set_checked(false);
                }
                item.set_checked(true);
                state.save_config();
                break;
            }
        }
    }
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
    menu.mode_items
        .iter()
        .chain(menu.line_actions_items.iter())
        .chain(menu.trim_items.iter())
        .chain(menu.escape_items.iter())
        .chain(menu.json_format_items.iter())
        .chain(menu.json_to_yaml_items.iter())
        .chain(menu.yaml_to_json_items.iter())
        .chain(menu.datetime_items.iter())
        .chain(menu.number_items.iter())
        .for_each(|(item, m)| item.set_checked(*m == mode));

    state.save_config();
    if let Some(processed) = process_clipboard(clipboard, mode) {
        state.set_last_processed_text(processed.clone());
        show_process_notification(mode, &processed);
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
    for (item, m) in &menu.monitor_mode_items {
        item.set_checked(*m == monitor_mode);
    }

    // 監視周期メニューの有効/無効を切り替え
    update_monitor_mode_impl(menu, monitor_mode);

    state.save_config();

    // 監視スレッドを再起動（世代を更新することで旧スレッドを終了させる）
    spawn_monitor_thread(Arc::clone(state));
}

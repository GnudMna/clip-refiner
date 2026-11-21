use std::sync::Arc;
use std::sync::mpsc;

use super::monitor;
use super::{handle_menu_event, should_hide_selector_on_focus_loss, update_refine};

use crate::config::MonitorMode;
use crate::refiner::RefineMode;
use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, LockExt, test_app_state};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

fn with_test_worker(
    state: &Arc<AppState>,
) -> (Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>) {
    let (tx, rx) = mpsc::channel();
    let worker = ClipboardWorkerHandle::for_test(Arc::clone(state), tx);
    (worker, rx)
}

// ======================================================================
// フォーカス喪失
// ======================================================================
/// 表示直後 200ms 以内はフォーカス喪失を無視すること
#[test]
fn should_not_hide_selector_immediately_after_show() {
    assert!(!should_hide_selector_on_focus_loss(100));
    assert!(!should_hide_selector_on_focus_loss(200));
}

/// 200ms 超過後はフォーカス喪失で非表示にすること
#[test]
fn should_hide_selector_after_focus_loss_delay() {
    assert!(should_hide_selector_on_focus_loss(201));
}

// ======================================================================
// 加工モード・監視設定
// ======================================================================
/// `update_refine` が設定とワーカーコマンドを更新すること
#[test]
fn update_refine_updates_config_and_sends_command() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, rx) = with_test_worker(&state);

    update_refine(&state, &menu, &worker, RefineMode::JsonFormat, None);

    assert_eq!(state.with_config(|c| c.mode), RefineMode::JsonFormat);
    assert!(
        menu.refine
            .all_mode_items()
            .any(|(item, mode)| *mode == RefineMode::JsonFormat && item.is_checked())
    );
    match rx.recv().expect("ワーカーコマンドが送信される") {
        ClipboardCommand::ProcessMode(mode) => assert_eq!(mode, RefineMode::JsonFormat),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// `update_monitor_mode` が設定と監視周期メニューを更新すること
#[test]
fn update_monitor_mode_switches_to_event_and_disables_interval() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");

    monitor::update_monitor_mode(&state, &menu, MonitorMode::Event);

    assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);
    assert!(
        menu.monitor
            .items
            .iter()
            .any(|(item, mode)| *mode == MonitorMode::Event && item.is_checked())
    );
    assert!(!menu.interval.main_submenu.is_enabled());
}

/// 同一モードへの `update_monitor_mode` は no-op であること
#[test]
fn update_monitor_mode_noop_when_unchanged() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    state.with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);

    monitor::update_monitor_mode(&state, &menu, MonitorMode::Polling);

    assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Polling);
    assert!(menu.interval.main_submenu.is_enabled());
}

// ======================================================================
// メニューイベント
// ======================================================================
fn menu_event(id: &tray_icon::menu::MenuId) -> MenuEvent {
    MenuEvent { id: id.clone() }
}

/// 終了メニューで `ControlFlow::Exit` になること
#[test]
fn handle_menu_event_quit_exits() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(menu.quit_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(matches!(control_flow, ControlFlow::Exit));
}

/// 一時停止チェック ON で設定が一時停止になること
#[test]
fn handle_menu_event_pause_enables_paused() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    menu.pause_item.set_checked(true);
    handle_menu_event(
        &menu_event(menu.pause_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(state.with_config(|c| c.is_paused));
}

/// 履歴クリアで履歴が空になること
#[test]
fn handle_menu_event_history_clear() {
    let state = Arc::new(test_app_state());
    state.with_config_mut(|c| c.history_enabled = true);
    state.add_to_history("entry");
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    menu.refresh_history(&state)
        .expect("履歴メニューの更新に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(menu.history.clear_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert_eq!(state.history_len(), 0);
}

/// 登録クリップの「クリップボードを登録」でワーカーコマンドが送信されること
#[test]
fn handle_menu_event_clips_register_sends_command() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, rx) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(menu.clips.register_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(matches!(
        rx.recv().expect("ワーカーコマンドが送信される"),
        ClipboardCommand::RegisterClipFromClipboard
    ));
}

/// 履歴項目選択でクリップボードへテキスト送信すること
#[test]
fn handle_menu_event_history_select_sends_text() {
    let state = Arc::new(test_app_state());
    state.with_config_mut(|c| c.history_enabled = true);
    state.add_to_history("copied text");
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    menu.refresh_history(&state)
        .expect("履歴メニューの更新に失敗");
    let record_id = {
        let records = menu.history.records.lock_ignore_poison();
        records[0].0.clone()
    };
    let (worker, rx) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(&record_id),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    match rx.recv().expect("ワーカーコマンドが送信される") {
        ClipboardCommand::SetText(text) => assert_eq!(text.as_str(), "copied text"),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// 加工モード選択で設定とワーカーコマンドが更新されること
#[test]
fn handle_menu_event_refine_mode_change() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (item, mode) = menu
        .refine
        .all_mode_items()
        .find(|(_, m)| *m == RefineMode::JsonFormat)
        .expect("JsonFormat メニュー項目が存在する");
    let (worker, rx) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert_eq!(state.with_config(|c| c.mode), RefineMode::JsonFormat);
    assert!(item.is_checked());
    match rx.recv().expect("ワーカーコマンドが送信される") {
        ClipboardCommand::ProcessMode(received) => assert_eq!(received, *mode),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// 通知 ON で設定が更新されること
#[test]
fn handle_menu_event_notification_enabled() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    menu.notification.enabled_item.set_checked(true);
    handle_menu_event(
        &menu_event(menu.notification.enabled_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(state.with_config(|c| c.notification_settings.enabled));
}

/// クリップボード内容表示の切替で `notify_result` が更新されること
#[test]
fn handle_menu_event_notification_clipboard_content_toggle() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    menu.notification.notify_result_item.set_checked(true);
    handle_menu_event(
        &menu_event(menu.notification.notify_result_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(state.with_config(|c| c.notification_settings.notify_result));
}

/// 監視周期選択で `interval_ms` が更新されること
#[test]
fn handle_menu_event_interval_change() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (item, _) = menu
        .interval
        .items
        .iter()
        .find(|(_, ms)| *ms == 500)
        .expect("0.5秒の監視周期項目が存在する");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert_eq!(state.with_config(|c| c.interval_ms), 500);
    assert!(item.is_checked());
}

/// 監視方式メニューで Event モードへ切り替わること
#[test]
fn handle_menu_event_monitor_mode_change() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (item, _) = menu
        .monitor
        .items
        .iter()
        .find(|(_, mode)| *mode == MonitorMode::Event)
        .expect("イベント監視項目が存在する");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);
    assert!(!menu.interval.main_submenu.is_enabled());
}

/// 履歴有効化で設定が更新されること
#[test]
fn handle_menu_event_history_enabled() {
    let state = Arc::new(test_app_state());
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    menu.history.enabled_item.set_checked(true);
    handle_menu_event(
        &menu_event(menu.history.enabled_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(state.with_config(|c| c.history_enabled));
}

/// お気に入り登録メニューで現在モードが登録されること
#[test]
fn handle_menu_event_add_favorite_registers_current_mode() {
    let state = Arc::new(test_app_state());
    state.with_config_mut(|config| config.mode = RefineMode::Trim);
    let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
    let (worker, _) = with_test_worker(&state);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(menu.refine.add_favorite_item.id()),
        &menu,
        &state,
        &worker,
        None,
        &mut control_flow,
    );

    assert!(state.with_config(|config| config.is_favorite_mode(RefineMode::Trim)));
}

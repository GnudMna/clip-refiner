use super::{
    handle_menu_event, should_hide_selector_on_focus_loss, update_monitor_mode, update_refine,
};
use crate::config::MonitorMode;
use crate::refiner::RefineMode;
use crate::test_helpers::{ClipboardCommand, TrayTestContext};
use crate::tray::state::LockExt;

use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

fn menu_event(id: &tray_icon::menu::MenuId) -> MenuEvent {
    MenuEvent { id: id.clone() }
}

// ======================================================================
// テスト
// ======================================================================
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
// 変換モード・監視設定
// ======================================================================
/// `update_refine` が設定とワーカーコマンドを更新すること
#[test]
fn update_refine_updates_config_and_sends_command() {
    let ctx = TrayTestContext::new();

    update_refine(
        ctx.state(),
        ctx.menu(),
        ctx.worker(),
        RefineMode::JsonFormat,
        None,
    );

    assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::JsonFormat);
    assert!(
        ctx.menu()
            .refine
            .all_mode_items()
            .any(|(item, mode)| *mode == RefineMode::JsonFormat && item.is_checked())
    );
    match ctx.recv_command() {
        ClipboardCommand::ProcessMode(mode) => assert_eq!(mode, RefineMode::JsonFormat),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// `update_monitor_mode` が設定と監視周期メニューを更新すること
#[test]
fn update_monitor_mode_switches_to_event_and_disables_interval() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);

    update_monitor_mode(ctx.state(), ctx.menu(), MonitorMode::Event);

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Event
    );
    assert!(
        ctx.menu()
            .monitor
            .items
            .iter()
            .any(|(item, mode)| *mode == MonitorMode::Event && item.is_checked())
    );
    assert!(!ctx.menu().interval.main_submenu.is_enabled());
}

/// 同一モードへの `update_monitor_mode` は no-op であようにすること
#[test]
fn update_monitor_mode_noop_when_unchanged() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);

    update_monitor_mode(ctx.state(), ctx.menu(), MonitorMode::Polling);

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Polling
    );
    assert!(ctx.menu().interval.main_submenu.is_enabled());
}

// ======================================================================
// メニューイベント
// ======================================================================
/// 終了メニューで `ControlFlow::Exit` になるようにすること
#[test]
fn handle_menu_event_quit_exits() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().quit_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(matches!(control_flow, ControlFlow::Exit));
}

/// 一時停止チェック ON で設定が一時停止になるようにすること
#[test]
fn handle_menu_event_pause_enables_paused() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    ctx.menu().pause_item.set_checked(true);
    handle_menu_event(
        &menu_event(ctx.menu().pause_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(ctx.state().with_config(|c| c.is_paused));
}

/// 履歴クリアで履歴が空になるようにすること
#[test]
fn handle_menu_event_history_clear() {
    let state = crate::test_helpers::test_app_state_arc();
    state.with_config_mut(|c| c.history_enabled = true);
    state.add_to_history("entry");
    let ctx = TrayTestContext::from_state(state);
    ctx.menu()
        .refresh_history(ctx.state())
        .expect("履歴メニューの更新に失敗");
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().history.clear_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(ctx.state().clipboard_session().history_len(), 0);
}

/// 登録クリップの「クリップボードを登録」でワーカーコマンドが送信されるようにすること
#[test]
fn handle_menu_event_clips_register_sends_command() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().clips.register_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(matches!(
        ctx.recv_command(),
        ClipboardCommand::RegisterClipFromClipboard
    ));
}

/// 履歴項目選択でクリップボードへテキスト送信すること
#[test]
fn handle_menu_event_history_select_sends_text() {
    let state = crate::test_helpers::test_app_state_arc();
    state.with_config_mut(|c| c.history_enabled = true);
    state.add_to_history("copied text");
    let ctx = TrayTestContext::from_state(state);
    ctx.menu()
        .refresh_history(ctx.state())
        .expect("履歴メニューの更新に失敗");
    let record_id = {
        let records = ctx.menu().history.records.lock_ignore_poison();
        records[0].0.clone()
    };
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(&record_id),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    match ctx.recv_command() {
        ClipboardCommand::SetText(text) => assert_eq!(text.as_str(), "copied text"),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// 変換モード選択で設定とワーカーコマンドが更新されるようにすること
#[test]
fn handle_menu_event_refine_mode_change() {
    let ctx = TrayTestContext::new();
    let (item, mode) = ctx
        .menu()
        .refine
        .all_mode_items()
        .find(|(_, m)| *m == RefineMode::JsonFormat)
        .expect("JsonFormat メニュー項目が存在する");
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::JsonFormat);
    assert!(item.is_checked());
    match ctx.recv_command() {
        ClipboardCommand::ProcessMode(received) => assert_eq!(received, *mode),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// 通知 ON で設定が更新されるようにすること
#[test]
fn handle_menu_event_notification_enabled() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    ctx.menu().notification.enabled_item.set_checked(true);
    handle_menu_event(
        &menu_event(ctx.menu().notification.enabled_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(ctx.state().with_config(|c| c.notification_settings.enabled));
}

/// クリップボード内容表示の切替で `notify_result` が更新されるようにすること
#[test]
fn handle_menu_event_notification_clipboard_content_toggle() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    ctx.menu().notification.notify_result_item.set_checked(true);
    handle_menu_event(
        &menu_event(ctx.menu().notification.notify_result_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(
        ctx.state()
            .with_config(|c| c.notification_settings.notify_result)
    );
}

/// 監視周期選択で `interval_ms` が更新されるようにすること
#[test]
fn handle_menu_event_interval_change() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);
    let (item, _) = ctx
        .menu()
        .interval
        .items
        .iter()
        .find(|(_, ms)| *ms == 500)
        .expect("0.5秒の監視周期項目が存在する");
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(ctx.state().with_config(|c| c.interval_ms), 500);
    assert!(item.is_checked());
    assert!(!ctx.menu().interval.custom_item.is_checked());
}

/// カスタム周期項目は設定ウィンドウを開き、周期値は変えないようにすること
#[test]
fn handle_menu_event_custom_interval_opens_settings() {
    let ctx = TrayTestContext::with_config(|c| {
        c.monitor_mode = MonitorMode::Polling;
        c.interval_ms = 750;
    });
    assert!(ctx.menu().interval.custom_item.is_checked());
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().interval.custom_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(ctx.state().with_config(|c| c.interval_ms), 750);
    assert!(ctx.menu().interval.custom_item.is_checked());
}

/// 監視方式メニューで Event モードへ切り替わようにすること
#[test]
fn handle_menu_event_monitor_mode_change() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);
    let (item, _) = ctx
        .menu()
        .monitor
        .items
        .iter()
        .find(|(_, mode)| *mode == MonitorMode::Event)
        .expect("イベント監視項目が存在する");
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Event
    );
    assert!(!ctx.menu().interval.main_submenu.is_enabled());
}

/// 履歴有効化で設定が更新されるようにすること
#[test]
fn handle_menu_event_history_enabled() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    ctx.menu().history.enabled_item.set_checked(true);
    handle_menu_event(
        &menu_event(ctx.menu().history.enabled_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(ctx.state().with_config(|c| c.history_enabled));
}

/// 連続記録有効化で設定が更新され、既存クリップボードはキューへ積まれないようにすること
#[test]
fn handle_menu_event_continuous_record_enabled() {
    let ctx = TrayTestContext::with_config(|c| c.continuous_record_enabled = false);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().continuous_record.enabled_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(ctx.state().with_config(|c| c.continuous_record_enabled));
    assert!(ctx.menu().continuous_record.enabled_item.is_checked());
    assert!(!ctx.menu().refine.main_submenu.is_enabled());
    assert_eq!(
        ctx.menu().continuous_record.main_submenu.text(),
        "連続記録 (自動加工停止)"
    );
    ctx.assert_no_command();
}

/// 連続記録メニューから直近 1 件を取り消せるようにすること
#[test]
fn handle_menu_event_continuous_record_undo_last() {
    let state = crate::test_helpers::test_app_state_arc();
    state.enqueue_continuous_record("first");
    state.enqueue_continuous_record("second");
    let ctx = TrayTestContext::from_state(state);
    ctx.menu()
        .refresh_continuous_record(ctx.state())
        .expect("連続記録メニューの更新に失敗");
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().continuous_record.undo_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(ctx.state().clipboard_session().continuous_record_len(), 1);
    assert_eq!(
        ctx.state()
            .clipboard_session()
            .get_continuous_record_entry(0)
            .as_ref()
            .map(|s| s.as_str()),
        Some("first")
    );
    ctx.assert_no_command();
}

/// お気に入り登録メニューで現在モードが登録されるようにすること
#[test]
fn handle_menu_event_add_favorite_registers_current_mode() {
    let ctx = TrayTestContext::with_config(|config| config.mode = RefineMode::Trim);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().refine.add_favorite_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(
        ctx.state()
            .with_config(|config| config.is_favorite_mode(RefineMode::Trim))
    );
}

/// クリップボード監視再開メニューがアプリ制御イベントとして処理されるようにすること
#[test]
fn handle_menu_event_retry_clipboard_worker_is_handled() {
    let ctx = TrayTestContext::new();
    ctx.menu().set_clipboard_worker_retry_enabled(true);
    let mut control_flow = ControlFlow::Wait;

    handle_menu_event(
        &menu_event(ctx.menu().retry_clipboard_worker_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(matches!(control_flow, ControlFlow::Wait));
}

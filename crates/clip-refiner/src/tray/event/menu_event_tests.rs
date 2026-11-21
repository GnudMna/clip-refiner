use tao::event_loop::ControlFlow;
use tray_icon::menu::MenuEvent;

use super::{
    handle_menu_event, should_hide_selector_on_focus_loss, update_monitor_mode, update_refine,
};
use crate::config::MonitorMode;
use crate::refiner::RefineMode;
use crate::test_helpers::{ClipboardCommand, TrayTestContext};
use crate::tray::state::LockExt;

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

/// `update_monitor_mode` が監視方式を更新すること
#[test]
fn update_monitor_mode_switches_to_event() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);

    update_monitor_mode(ctx.state(), MonitorMode::Event);

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Event
    );
}

/// 同一モードへの `update_monitor_mode` は no-op であようにすること
#[test]
fn update_monitor_mode_noop_when_unchanged() {
    let ctx = TrayTestContext::with_config(|c| c.monitor_mode = MonitorMode::Polling);

    update_monitor_mode(ctx.state(), MonitorMode::Polling);

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Polling
    );
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

/// 自動変換メニューのチェック OFF で自動変換が無効になること
#[test]
fn handle_menu_event_auto_refine_toggle_disables_auto_refine() {
    let ctx = TrayTestContext::new();
    let mut control_flow = ControlFlow::Wait;

    ctx.menu().refine.auto_refine_item.set_checked(false);
    handle_menu_event(
        &menu_event(ctx.menu().refine.auto_refine_item.id()),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(!ctx.state().with_config(|c| c.auto_refine_enabled));
    assert!(!ctx.state().monitor_snapshot().auto_processing_enabled());
}

/// 自動変換が無効でも変換モードメニューを選ぶと自動変換が有効になること
#[test]
fn handle_menu_event_mode_selection_enables_auto_refine() {
    let ctx = TrayTestContext::with_config(|c| c.auto_refine_enabled = false);
    let mut control_flow = ControlFlow::Wait;

    let id = ctx
        .menu()
        .refine
        .all_mode_items()
        .find(|(_, mode)| *mode == RefineMode::JsonFormat)
        .map(|(item, _)| item.id().clone())
        .expect("JsonFormat のメニュー項目");
    handle_menu_event(
        &menu_event(&id),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert!(ctx.state().with_config(|c| c.auto_refine_enabled));
    assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::JsonFormat);
    assert!(ctx.menu().refine.auto_refine_item.is_checked());
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
    assert!(!ctx.menu().regex_presets.main_submenu.is_enabled());
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

/// 正規表現プリセットメニューで有効プリセットを切り替えられるようにすること
#[test]
fn handle_menu_event_regex_preset_switches_active() {
    use crate::config::{RegexPreset, RegexSettings, RegexStep};

    let ctx = TrayTestContext::with_config(|config| {
        config.regex = RegexSettings {
            active_preset_id: "a".to_string(),
            presets: vec![
                RegexPreset {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    steps: vec![RegexStep::default()],
                },
                RegexPreset {
                    id: "b".to_string(),
                    name: "B".to_string(),
                    steps: vec![RegexStep::default()],
                },
            ],
        };
    });
    ctx.menu()
        .refresh_regex_presets(ctx.state())
        .expect("正規表現プリセットメニューの更新に失敗");
    let mut control_flow = ControlFlow::Wait;

    let target_id = {
        let records = ctx.menu().regex_presets.records.lock_ignore_poison();
        records
            .iter()
            .find(|(_, id)| id == "b")
            .map(|(item, _)| item.id().clone())
            .expect("プリセット B のメニュー項目があること")
    };

    handle_menu_event(
        &menu_event(&target_id),
        ctx.menu(),
        ctx.state(),
        ctx.worker(),
        None,
        &mut control_flow,
    );

    assert_eq!(
        ctx.state()
            .with_config(|c| c.regex.active_preset_id.clone()),
        "b"
    );
    let records = ctx.menu().regex_presets.records.lock_ignore_poison();
    assert!(
        records
            .iter()
            .any(|(item, id)| id == "b" && item.is_checked())
    );
    assert!(
        records
            .iter()
            .any(|(item, id)| id == "a" && !item.is_checked())
    );
}

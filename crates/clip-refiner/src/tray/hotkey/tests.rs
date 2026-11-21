use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use super::*;

use crate::config::HotkeySettings;
use crate::refiner::RefineMode;
use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, test_app_state};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use global_hotkey::HotKeyState;
use tao::event_loop::ControlFlow;

// ======================================================================
// テスト用コンテキスト
// ======================================================================
struct HotkeyTestContext {
    state: Arc<AppState>,
    menu: TrayMenu,
    control_flow: ControlFlow,
    last_quick_selector_show: Instant,
    last_clip_selector_show: Instant,
    clipboard_worker: Arc<ClipboardWorkerHandle>,
}

impl HotkeyTestContext {
    fn new() -> Self {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (tx, _) = mpsc::channel();
        let clipboard_worker = ClipboardWorkerHandle::for_test(Arc::clone(&state), tx);

        Self {
            state,
            menu,
            control_flow: ControlFlow::Wait,
            last_quick_selector_show: Instant::now(),
            last_clip_selector_show: Instant::now(),
            clipboard_worker,
        }
    }

    fn ctx(&mut self) -> HotkeyEventContext<'_> {
        HotkeyEventContext {
            state: &self.state,
            menu: &self.menu,
            quick_selector: None,
            clip_selector: None,
            ocr_capture: None,
            control_flow: &mut self.control_flow,
            last_quick_selector_show: &mut self.last_quick_selector_show,
            last_clip_selector_show: &mut self.last_clip_selector_show,
            clipboard_worker: &self.clipboard_worker,
        }
    }
}

/// テスト間でホットキー登録が衝突しないよう F キーを割り当てる
fn test_hotkeys() -> HotkeySettings {
    HotkeySettings {
        quick_selector: "Alt+Shift+F1".to_string(),
        notification: "Alt+Shift+F2".to_string(),
        pause: "Alt+Shift+F3".to_string(),
        quit: "Alt+Shift+F4".to_string(),
        undo: "Alt+Shift+F5".to_string(),
        clip_selector: "Alt+Shift+F6".to_string(),
        ocr: "Alt+Shift+F7".to_string(),
        continuous_record: "Alt+Shift+F8".to_string(),
        favorite_mode_slots: Vec::new(),
    }
}

fn press_event(hotkey_id: u32) -> GlobalHotKeyEvent {
    GlobalHotKeyEvent {
        id: hotkey_id,
        state: HotKeyState::Pressed,
    }
}

fn release_event(hotkey_id: u32) -> GlobalHotKeyEvent {
    GlobalHotKeyEvent {
        id: hotkey_id,
        state: HotKeyState::Released,
    }
}

fn assert_release_ignored(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    handler.handle_event(release_event(handler.quit_hotkey_id()), &mut ctx.ctx());
    assert!(matches!(ctx.control_flow, ControlFlow::Wait));
}

fn assert_pause_toggle(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    assert!(!ctx.state.with_config(|c| c.is_paused));
    handler.handle_event(press_event(handler.pause_hotkey_id()), &mut ctx.ctx());
    assert!(ctx.state.with_config(|c| c.is_paused));
    assert!(ctx.menu.pause_item.is_checked());
}

fn assert_notification_toggle(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    assert!(!ctx.state.with_config(|c| c.notification_settings.enabled));
    handler.handle_event(
        press_event(handler.notification_hotkey_id()),
        &mut ctx.ctx(),
    );
    assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
    assert!(ctx.menu.notification.enabled_item.is_checked());
}

fn assert_continuous_record_toggle(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    assert!(!ctx.state.with_config(|c| c.continuous_record_enabled));
    handler.handle_event(
        press_event(handler.continuous_record_hotkey_id()),
        &mut ctx.ctx(),
    );
    assert!(ctx.state.with_config(|c| c.continuous_record_enabled));
    assert!(ctx.menu.continuous_record.enabled_item.is_checked());
}

fn assert_undo_sends_command(
    handler: &mut HotkeyHandler,
    ctx: &mut HotkeyTestContext,
) -> mpsc::Receiver<ClipboardCommand> {
    let (tx, rx) = mpsc::channel();
    ctx.clipboard_worker = ClipboardWorkerHandle::for_test(Arc::clone(&ctx.state), tx);
    handler.handle_event(press_event(handler.undo_hotkey_id()), &mut ctx.ctx());
    assert!(matches!(
        rx.recv().expect("ワーカーコマンドが送信される"),
        ClipboardCommand::Undo
    ));
    rx
}

fn assert_favorite_hotkeys(
    handler: &mut HotkeyHandler,
    ctx: &mut HotkeyTestContext,
    rx: &mpsc::Receiver<ClipboardCommand>,
) {
    ctx.state.with_config_mut(|config| {
        config.favorite_modes = vec![RefineMode::Trim, RefineMode::JsonFormat];
    });
    handler
        .reload_favorite_slots(&test_hotkeys(), 2)
        .expect("お気に入りホットキーの再登録に失敗");

    let favorite_slot_one = handler
        .favorite_hotkey_id_at(1)
        .expect("スロット 1 のホットキーが登録される");
    handler.handle_event(press_event(favorite_slot_one), &mut ctx.ctx());
    assert_eq!(
        ctx.state.with_config(|config| config.mode),
        RefineMode::JsonFormat
    );
    assert!(matches!(
        rx.recv()
            .expect("お気に入りホットキーで加工コマンドが送信される"),
        ClipboardCommand::ProcessMode(RefineMode::JsonFormat)
    ));

    let favorite_slot_zero = handler
        .favorite_hotkey_id_at(0)
        .expect("スロット 0 のホットキーが登録される");
    handler.handle_event(press_event(favorite_slot_zero), &mut ctx.ctx());
    assert_eq!(
        ctx.state.with_config(|config| config.mode),
        RefineMode::Trim
    );
    assert!(matches!(
        rx.recv().expect("スロット 0 で加工コマンドが送信される"),
        ClipboardCommand::ProcessMode(RefineMode::Trim)
    ));
}

fn assert_quit_exits(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    handler.handle_event(press_event(handler.quit_hotkey_id()), &mut ctx.ctx());
    assert!(matches!(ctx.control_flow, ControlFlow::Exit));
}

// ======================================================================
// テスト
// ======================================================================

/// ホットキーイベント処理の一連の挙動を検証すること
///
/// グローバルホットキーは OS に残るため 1 テストにまとめる
#[test]
fn hotkey_event_handling() {
    let mut handler = HotkeyHandler::new(&test_hotkeys(), &[])
        .expect("テスト用ホットキーの登録に失敗")
        .0;
    let mut ctx = HotkeyTestContext::new();

    assert_release_ignored(&mut handler, &mut ctx);
    assert_pause_toggle(&mut handler, &mut ctx);
    assert_notification_toggle(&mut handler, &mut ctx);
    assert_continuous_record_toggle(&mut handler, &mut ctx);
    let rx = assert_undo_sends_command(&mut handler, &mut ctx);
    assert_favorite_hotkeys(&mut handler, &mut ctx, &rx);
    assert_quit_exits(&mut handler, &mut ctx);
}

/// `toggle_pause` が設定とメニューを更新すること
#[test]
fn toggle_pause_updates_state_and_menu() {
    let mut ctx = HotkeyTestContext::new();
    HotkeyHandler::toggle_pause(&mut ctx.ctx());
    assert!(ctx.state.with_config(|c| c.is_paused));
    assert!(ctx.menu.pause_item.is_checked());
}

/// `toggle_continuous_record` が設定とメニューを更新すること
#[test]
fn toggle_continuous_record_updates_state_and_menu() {
    let mut ctx = HotkeyTestContext::new();
    HotkeyHandler::toggle_continuous_record(&mut ctx.ctx());
    assert!(ctx.state.with_config(|c| c.continuous_record_enabled));
    assert!(ctx.menu.continuous_record.enabled_item.is_checked());
}

/// `toggle_notification` が設定とメニューを更新すること
#[test]
fn toggle_notification_updates_state_and_menu() {
    let mut ctx = HotkeyTestContext::new();
    HotkeyHandler::toggle_notification(&mut ctx.ctx());
    assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
    assert!(ctx.menu.notification.enabled_item.is_checked());
}

/// `reload` でホットキー ID が更新されるようにすること
#[test]
fn reload_updates_registered_hotkey_ids() {
    let initial = HotkeySettings {
        quick_selector: "Alt+Ctrl+F1".to_string(),
        notification: "Alt+Ctrl+F2".to_string(),
        pause: "Alt+Ctrl+F3".to_string(),
        quit: "Alt+Ctrl+F4".to_string(),
        undo: "Alt+Ctrl+F5".to_string(),
        clip_selector: "Alt+Ctrl+F6".to_string(),
        ocr: "Alt+Ctrl+F7".to_string(),
        continuous_record: "Alt+Ctrl+F8".to_string(),
        favorite_mode_slots: Vec::new(),
    };
    let mut handler = HotkeyHandler::new(&initial, &[])
        .expect("テスト用ホットキーの登録に失敗")
        .0;
    let quit_before = handler.quit_hotkey_id();

    let mut updated = initial.clone();
    updated.quit = "Alt+Ctrl+F12".to_string();
    handler
        .reload(&updated, &[])
        .expect("ホットキーの再登録に失敗");

    assert_ne!(handler.quit_hotkey_id(), quit_before);

    let ocr_before = handler.ocr_hotkey_id();
    updated.ocr = "Alt+Ctrl+F9".to_string();
    handler
        .reload(&updated, &[])
        .expect("OCR ホットキーの再登録に失敗");
    assert_ne!(handler.ocr_hotkey_id(), ocr_before);
}

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use super::*;

use crate::config::HotkeySettings;
use crate::refiner::RefineMode;
use crate::test_helpers::{stub_worker, test_app_state_arc, test_tray_menu};
use crate::tray::menu::TrayMenu;
use crate::tray::ocr::{OcrCaptureWindow, init_ocr_capture};
use crate::tray::selector::{ClipSelectorWindow, QuickSelectorWindow};
use crate::tray::state::AppState;
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
    quick_selector: Option<QuickSelectorWindow>,
    clip_selector: Option<ClipSelectorWindow>,
    ocr_capture: Option<OcrCaptureWindow>,
}

impl HotkeyTestContext {
    fn new() -> Self {
        let state = test_app_state_arc();
        let menu = test_tray_menu(&state);
        let (clipboard_worker, _) = stub_worker(&state);

        Self {
            state,
            menu,
            control_flow: ControlFlow::Wait,
            last_quick_selector_show: Instant::now(),
            last_clip_selector_show: Instant::now(),
            clipboard_worker,
            quick_selector: None,
            clip_selector: None,
            ocr_capture: None,
        }
    }

    /// セレクター / OCR の空シェル付きコンテキストを作る (実 `WebView` / HWND は作らない)
    fn with_ui_shells() -> Self {
        let mut ctx = Self::new();
        ctx.quick_selector = Some(QuickSelectorWindow::new_shell(ctx.state.proxy.clone()));
        ctx.clip_selector = Some(ClipSelectorWindow::new_shell(ctx.state.proxy.clone()));
        let ocr = init_ocr_capture(&ctx.state.proxy, Arc::clone(&ctx.clipboard_worker));
        ocr.enable_show_simulation_for_test();
        ctx.ocr_capture = Some(ocr);
        ctx
    }

    fn ctx(&mut self) -> HotkeyEventContext<'_> {
        HotkeyEventContext {
            state: &self.state,
            menu: &self.menu,
            quick_selector: self.quick_selector.as_ref(),
            clip_selector: self.clip_selector.as_ref(),
            ocr_capture: self.ocr_capture.as_ref(),
            event_loop: None,
            control_flow: &mut self.control_flow,
            last_quick_selector_show: &mut self.last_quick_selector_show,
            last_clip_selector_show: &mut self.last_clip_selector_show,
            clipboard_worker: &self.clipboard_worker,
        }
    }
}

/// 本番デフォルトや他アプリと衝突しにくい F キーを割り当てる
///
/// `Alt+Shift+F10` は環境によって OS / 他プロセスが占有していることがあるため使わない
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
        continuous_record_undo: "Alt+Shift+F9".to_string(),
        continuous_record_paste: "Alt+Shift+F12".to_string(),
        favorite_mode_slots: Vec::new(),
    }
}

/// UI 相互排他テスト用 (既存テストの登録と衝突しないキー)
fn ui_test_hotkeys() -> HotkeySettings {
    HotkeySettings {
        quick_selector: "Alt+Ctrl+Shift+F1".to_string(),
        notification: "Alt+Ctrl+Shift+F2".to_string(),
        pause: "Alt+Ctrl+Shift+F3".to_string(),
        quit: "Alt+Ctrl+Shift+F4".to_string(),
        undo: "Alt+Ctrl+Shift+F5".to_string(),
        clip_selector: "Alt+Ctrl+Shift+F6".to_string(),
        ocr: "Alt+Ctrl+Shift+F7".to_string(),
        continuous_record: "Alt+Ctrl+Shift+F8".to_string(),
        continuous_record_undo: "Alt+Ctrl+Shift+F9".to_string(),
        continuous_record_paste: "Alt+Ctrl+Shift+F10".to_string(),
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

fn assert_continuous_record_undo(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    ctx.state.enqueue_continuous_record("first");
    ctx.state.enqueue_continuous_record("second");
    assert_eq!(ctx.state.clipboard_session().continuous_record_len(), 2);

    handler.handle_event(
        press_event(handler.continuous_record_undo_hotkey_id()),
        &mut ctx.ctx(),
    );
    assert_eq!(ctx.state.clipboard_session().continuous_record_len(), 1);
    assert_eq!(
        ctx.state
            .clipboard_session()
            .get_continuous_record_entry(0)
            .as_ref()
            .map(|s| s.as_str()),
        Some("first")
    );
}

fn assert_continuous_record_paste(handler: &mut HotkeyHandler, ctx: &mut HotkeyTestContext) {
    assert_eq!(ctx.state.clipboard_session().continuous_record_len(), 1);

    let (tx, rx) = mpsc::channel();
    ctx.clipboard_worker = ClipboardWorkerHandle::for_test(Arc::clone(&ctx.state), tx);
    handler.handle_event(
        press_event(handler.continuous_record_paste_hotkey_id()),
        &mut ctx.ctx(),
    );
    assert!(matches!(
        rx.recv().expect("連続記録貼り付けコマンドが送信される"),
        ClipboardCommand::PasteContinuousRecordFront
    ));
    // キュー排出はワーカー処理後。ここではコマンド送出のみを検証する
    assert_eq!(ctx.state.clipboard_session().continuous_record_len(), 1);
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
    assert_continuous_record_undo(&mut handler, &mut ctx);
    assert_continuous_record_paste(&mut handler, &mut ctx);
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
        continuous_record_undo: "Alt+Ctrl+F10".to_string(),
        continuous_record_paste: "Alt+Ctrl+F9".to_string(),
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
    updated.ocr = "Alt+Ctrl+F11".to_string();
    handler
        .reload(&updated, &[])
        .expect("OCR ホットキーの再登録に失敗");
    assert_ne!(handler.ocr_hotkey_id(), ocr_before);
}

/// OCR / セレクターホットキーの表示トグルと相互排他を検証すること
///
/// グローバルホットキーは OS に残るため 1 テストにまとめる。実 `WebView` / HWND は作らない
#[test]
fn hotkey_ui_overlay_mutual_exclusion() {
    let handler = HotkeyHandler::new(&ui_test_hotkeys(), &[])
        .expect("テスト用ホットキーの登録に失敗")
        .0;
    let mut ctx = HotkeyTestContext::with_ui_shells();
    let ocr_id = handler.ocr_hotkey_id();
    let quick_id = handler.quick_selector_hotkey_id();
    let clip_id = handler.clip_selector_hotkey_id();

    // OCR: 非表示 → 擬似表示 → 非表示
    assert!(!ctx.ocr_capture.as_ref().unwrap().is_visible());
    handler.handle_event(press_event(ocr_id), &mut ctx.ctx());
    assert!(ctx.ocr_capture.as_ref().unwrap().is_visible());
    handler.handle_event(press_event(ocr_id), &mut ctx.ctx());
    assert!(!ctx.ocr_capture.as_ref().unwrap().is_visible());

    // OCR 表示前にセレクターを閉じる
    ctx.quick_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    ctx.clip_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    handler.handle_event(press_event(ocr_id), &mut ctx.ctx());
    assert!(!ctx.quick_selector.as_ref().unwrap().is_visible());
    assert!(!ctx.clip_selector.as_ref().unwrap().is_visible());
    assert!(ctx.ocr_capture.as_ref().unwrap().is_visible());
    ctx.ocr_capture.as_ref().unwrap().hide();

    // クリップセレクター押下でクイックを閉じる (event_loop 無しのため表示はしない)
    ctx.quick_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    handler.handle_event(press_event(clip_id), &mut ctx.ctx());
    assert!(!ctx.quick_selector.as_ref().unwrap().is_visible());

    // クイックセレクター押下でクリップを閉じる
    ctx.clip_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    handler.handle_event(press_event(quick_id), &mut ctx.ctx());
    assert!(!ctx.clip_selector.as_ref().unwrap().is_visible());

    // 表示中のセレクターは再押下で閉じる
    ctx.quick_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    handler.handle_event(press_event(quick_id), &mut ctx.ctx());
    assert!(!ctx.quick_selector.as_ref().unwrap().is_visible());

    ctx.clip_selector
        .as_ref()
        .unwrap()
        .force_visible_for_test(true);
    handler.handle_event(press_event(clip_id), &mut ctx.ctx());
    assert!(!ctx.clip_selector.as_ref().unwrap().is_visible());
}

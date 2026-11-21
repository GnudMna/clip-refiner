use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use global_hotkey::HotKeyState;
use tao::event_loop::ControlFlow;

use super::register::HotkeyRegistrationFailure;
use super::*;
use crate::config::HotkeySettings;
use crate::refiner::RefineMode;
use crate::test_helpers::{stub_worker, test_app_state_arc, test_tray_menu};
use crate::tray::menu::TrayMenu;
use crate::tray::ocr::{OcrCaptureWindow, init_ocr_capture};
use crate::tray::selector::{ClipSelectorWindow, QuickSelectorWindow};
use crate::tray::state::AppState;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

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

/// 常駐中の本体や他アプリと衝突しにくいキーを割り当てる
///
/// 本体の既定 (固定キー・お気に入りスロットとも `Alt+Shift` 系) と重ならないよう
/// テスト用キーは `Alt+Ctrl+Shift` 系に限り、並列実行される他テストとも重ならない範囲を使う。
/// お気に入りスロットは未指定だと既定キーになるため明示する
fn test_hotkeys() -> HotkeySettings {
    HotkeySettings {
        quick_selector: "Alt+Ctrl+Shift+A".to_string(),
        notification: "Alt+Ctrl+Shift+B".to_string(),
        pause: "Alt+Ctrl+Shift+C".to_string(),
        quit: "Alt+Ctrl+Shift+D".to_string(),
        undo: "Alt+Ctrl+Shift+E".to_string(),
        clip_selector: "Alt+Ctrl+Shift+F".to_string(),
        ocr: "Alt+Ctrl+Shift+G".to_string(),
        continuous_record: "Alt+Ctrl+Shift+H".to_string(),
        continuous_record_undo: "Alt+Ctrl+Shift+I".to_string(),
        continuous_record_paste: "Alt+Ctrl+Shift+J".to_string(),
        favorite_mode_slots: vec![
            "Alt+Ctrl+Shift+K".to_string(),
            "Alt+Ctrl+Shift+L".to_string(),
        ],
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

/// ホットキー登録の失敗が無いことを検証する
///
/// 他プロセスが同じキーを占有していると登録に失敗するため、どのキーが何の理由で失敗したかを出す
fn assert_registered(failures: &[HotkeyRegistrationFailure], context: &str) {
    assert!(
        failures.is_empty(),
        "{context}: {}
常駐中の ClipRefiner や他アプリが同じキーを登録していないか確認する",
        format_registration_failures(failures)
    );
}

/// テスト用ホットキーでハンドラを作り、全キーが登録されたことを検証する
fn new_registered_handler(hotkeys: &HotkeySettings) -> HotkeyHandler {
    let (handler, failures) =
        HotkeyHandler::new(hotkeys, &[]).expect("テスト用ホットキーハンドラの初期化に失敗");
    assert_registered(&failures, "テスト用ホットキーの登録に失敗");
    handler
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
        config.mode = RefineMode::UrlDecode;
        config.pipeline = vec![RefineMode::UrlDecode, RefineMode::Trim];
    });
    let failures = handler
        .reload_favorite_slots(&test_hotkeys(), 2)
        .expect("お気に入りホットキーの登録解除に失敗");
    assert_registered(&failures, "お気に入りホットキーの登録に失敗");

    let favorite_slot_one = handler
        .favorite_hotkey_id_at(1)
        .expect("スロット 1 のホットキーが登録される");
    handler.handle_event(press_event(favorite_slot_one), &mut ctx.ctx());
    // 一回適用のため自動変換の設定は変わらない
    assert_auto_refine_unchanged(ctx);
    assert!(matches!(
        rx.recv()
            .expect("お気に入りホットキーで加工コマンドが送信される"),
        ClipboardCommand::ProcessMode(RefineMode::JsonFormat)
    ));

    let favorite_slot_zero = handler
        .favorite_hotkey_id_at(0)
        .expect("スロット 0 のホットキーが登録される");
    handler.handle_event(press_event(favorite_slot_zero), &mut ctx.ctx());
    assert_auto_refine_unchanged(ctx);
    assert!(matches!(
        rx.recv().expect("スロット 0 で加工コマンドが送信される"),
        ClipboardCommand::ProcessMode(RefineMode::Trim)
    ));
}

fn assert_auto_refine_unchanged(ctx: &HotkeyTestContext) {
    let (mode, pipeline) = ctx
        .state
        .with_config(|config| (config.mode, config.pipeline.clone()));
    assert_eq!(mode, RefineMode::UrlDecode);
    assert_eq!(pipeline, vec![RefineMode::UrlDecode, RefineMode::Trim]);
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
    let mut handler = new_registered_handler(&test_hotkeys());
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

/// `toggle_notification` が設定を更新すること
#[test]
fn toggle_notification_updates_state() {
    let mut ctx = HotkeyTestContext::new();
    HotkeyHandler::toggle_notification(&mut ctx.ctx());
    assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
}

/// `reload` でホットキー ID が更新されるようにすること
#[test]
fn reload_updates_registered_hotkey_ids() {
    let initial = HotkeySettings {
        quick_selector: "Alt+Ctrl+Shift+M".to_string(),
        notification: "Alt+Ctrl+Shift+N".to_string(),
        pause: "Alt+Ctrl+Shift+O".to_string(),
        quit: "Alt+Ctrl+Shift+P".to_string(),
        undo: "Alt+Ctrl+Shift+Q".to_string(),
        clip_selector: "Alt+Ctrl+Shift+R".to_string(),
        ocr: "Alt+Ctrl+Shift+S".to_string(),
        continuous_record: "Alt+Ctrl+Shift+T".to_string(),
        continuous_record_undo: "Alt+Ctrl+Shift+U".to_string(),
        continuous_record_paste: "Alt+Ctrl+Shift+V".to_string(),
        favorite_mode_slots: Vec::new(),
    };
    let mut handler = new_registered_handler(&initial);
    let quit_before = handler.quit_hotkey_id();

    let mut updated = initial.clone();
    updated.quit = "Alt+Ctrl+Shift+W".to_string();
    let failures = handler
        .reload(&updated, &[])
        .expect("ホットキーの登録解除に失敗");
    assert_registered(&failures, "ホットキーの再登録に失敗");

    assert_ne!(handler.quit_hotkey_id(), quit_before);

    let ocr_before = handler.ocr_hotkey_id();
    updated.ocr = "Alt+Ctrl+Shift+Y".to_string();
    let failures = handler
        .reload(&updated, &[])
        .expect("ホットキーの登録解除に失敗");
    assert_registered(&failures, "OCR ホットキーの再登録に失敗");
    assert_ne!(handler.ocr_hotkey_id(), ocr_before);
}

/// OCR / セレクターホットキーの表示トグルと相互排他を検証すること
///
/// グローバルホットキーは OS に残るため 1 テストにまとめる。実 `WebView` / HWND は作らない
#[test]
fn hotkey_ui_overlay_mutual_exclusion() {
    let handler = new_registered_handler(&ui_test_hotkeys());
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

/// ホットキー受信タイムアウトが短い応答性向けの値となるようにすること
#[test]
fn hotkey_recv_timeout_is_responsive() {
    assert_eq!(HOTKEY_RECV_TIMEOUT.as_millis(), 10);
}

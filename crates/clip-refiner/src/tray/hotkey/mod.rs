//! グローバルホットキーの解決・登録・イベント処理

mod convert;
mod handler;
mod register;
mod resolve;
pub(crate) mod settings;

pub use handler::HotkeyEventContext;
pub use register::HotkeyHandler;
pub(crate) use register::{format_registration_failures, notify_registration_failures};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::dispatch;
use super::state::AppEvent;

use global_hotkey::GlobalHotKeyEvent;
use tao::event_loop::EventLoopProxy;

// ======================================================================
// リスナー停止制御
// ======================================================================
/// ホットキーイベントリスナースレッドの停止フラグと join ハンドル
pub(crate) struct HotkeyListenerStop {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HotkeyListenerStop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl HotkeyHandler {
    /// ホットキーイベントを受信するためのバックグラウンドスレッドを開始する
    ///
    /// 受信したイベントは `proxy` を介してメインのイベントループへ転送される
    ///
    /// # Arguments
    /// * `proxy` - UI スレッド (イベントループ) へイベントを送信するためのプロキシ
    ///
    /// # Returns
    /// * `HotkeyListenerStop` - リスナースレッドの停止制御
    pub fn start_event_listener(proxy: EventLoopProxy<AppEvent>) -> HotkeyListenerStop {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while !stop_for_thread.load(Ordering::Relaxed) {
                let mut had_hotkey_event = false;
                while let Ok(event) = receiver.try_recv() {
                    had_hotkey_event = true;
                    dispatch::send_app_event(&proxy, AppEvent::Hotkey(event));
                }
                if !had_hotkey_event {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        HotkeyListenerStop {
            stop,
            thread: Some(thread),
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
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

    /// ホットキーイベント処理の一連の挙動を検検証すること
    ///
    /// グローバルホットキーは OS に残るため 1 テストにまとめる
    #[test]
    #[allow(clippy::too_many_lines)]
    fn hotkey_event_handling() {
        let mut handler = HotkeyHandler::new(&test_hotkeys(), &[])
            .expect("テスト用ホットキーの登録に失敗")
            .0;
        let mut ctx = HotkeyTestContext::new();

        // キー解放は無視
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.quit_hotkey_id(),
                state: HotKeyState::Released,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(ctx.control_flow, ControlFlow::Wait));

        // 一時停止
        assert!(!ctx.state.with_config(|c| c.is_paused));
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.pause_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(ctx.state.with_config(|c| c.is_paused));
        assert!(ctx.menu.pause_item.is_checked());

        // 通知
        assert!(!ctx.state.with_config(|c| c.notification_settings.enabled));
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.notification_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(ctx.state.with_config(|c| c.notification_settings.enabled));
        assert!(ctx.menu.notification.enabled_item.is_checked());

        // 連続記録
        assert!(!ctx.state.with_config(|c| c.continuous_record_enabled));
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.continuous_record_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(ctx.state.with_config(|c| c.continuous_record_enabled));
        assert!(ctx.menu.continuous_record.enabled_item.is_checked());

        // 取り消し
        let (tx, rx) = mpsc::channel();
        ctx.clipboard_worker = ClipboardWorkerHandle::for_test(Arc::clone(&ctx.state), tx);
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.undo_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(
            rx.recv().expect("ワーカーコマンドが送信される"),
            ClipboardCommand::Undo
        ));

        // お気に入りホットキー (スロット 1)
        ctx.state.with_config_mut(|config| {
            config.favorite_modes = vec![RefineMode::Trim, RefineMode::JsonFormat];
        });
        handler
            .reload_favorite_slots(&test_hotkeys(), 2)
            .expect("お気に入りホットキーの再登録に失敗");
        let favorite_slot_one = handler
            .favorite_hotkey_id_at(1)
            .expect("スロット 1 のホットキーが登録される");
        handler.handle_event(
            GlobalHotKeyEvent {
                id: favorite_slot_one,
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert_eq!(
            ctx.state.with_config(|config| config.mode),
            RefineMode::JsonFormat
        );
        assert!(matches!(
            rx.recv()
                .expect("お気に入りホットキーで加工コマンドが送信される"),
            ClipboardCommand::ProcessMode(RefineMode::JsonFormat)
        ));

        // お気に入りホットキー (スロット 0)
        let favorite_slot_zero = handler
            .favorite_hotkey_id_at(0)
            .expect("スロット 0 のホットキーが登録される");
        handler.handle_event(
            GlobalHotKeyEvent {
                id: favorite_slot_zero,
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert_eq!(
            ctx.state.with_config(|config| config.mode),
            RefineMode::Trim
        );
        assert!(matches!(
            rx.recv().expect("スロット 0 で加工コマンドが送信される"),
            ClipboardCommand::ProcessMode(RefineMode::Trim)
        ));

        // 終了
        handler.handle_event(
            GlobalHotKeyEvent {
                id: handler.quit_hotkey_id(),
                state: HotKeyState::Pressed,
            },
            &mut ctx.ctx(),
        );
        assert!(matches!(ctx.control_flow, ControlFlow::Exit));
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
}

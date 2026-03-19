use std::sync::Arc;

use super::menu::TrayMenu;
use super::notifier;
use super::selector::SelectorWindow;
use super::state::{AppEvent, AppState};
use crate::notification;

use anyhow::Result;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use tao::event_loop::{ControlFlow, EventLoopProxy};

/// グローバルホットキーを管理する構造体
pub struct HotkeyHandler {
    _manager: GlobalHotKeyManager,
    selector_hotkey: HotKey,
    notification_hotkey: HotKey,
    pause_hotkey: HotKey,
    quit_hotkey: HotKey,
}

impl HotkeyHandler {
    /// ホットキーハンドラを初期化し、ショートカットを登録する。
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `HotkeyHandler` インスタンス。
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!(e))?;
        let selector_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);
        let notification_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyN);
        let pause_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyP);
        let quit_hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyQ);

        let register = |hotkey| manager.register(hotkey).map_err(|e| anyhow::anyhow!(e));

        register(selector_hotkey)?;
        register(notification_hotkey)?;
        register(pause_hotkey)?;
        register(quit_hotkey)?;

        Ok(Self {
            _manager: manager,
            selector_hotkey,
            notification_hotkey,
            pause_hotkey,
            quit_hotkey,
        })
    }

    /// ホットキーイベントを監視するスレッドを開始する。
    ///
    /// # Arguments
    /// * `proxy` - UIイベントを送信するためのプロキシ。
    pub fn start_event_listener(&self, proxy: EventLoopProxy<AppEvent>) {
        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                let _ = proxy.send_event(AppEvent::Hotkey(event));
            }
        });
    }

    /// ホットキーイベントを処理する。
    ///
    /// # Arguments
    /// * `event` - 受信したホットキーイベント。
    /// * `state` - アプリケーションの状態。
    /// * `menu` - トレイメニュー。
    /// * `selector` - セレクターウィンドウ。
    /// * `control_flow` - イベントループの制御フロー。
    /// * `last_selector_show` - セレクターが最後に表示された時刻。
    pub fn handle_event(
        &self,
        event: GlobalHotKeyEvent,
        state: &Arc<AppState>,
        menu: &TrayMenu,
        selector: &SelectorWindow,
        control_flow: &mut ControlFlow,
        last_selector_show: &mut std::time::Instant,
    ) {
        if event.state == global_hotkey::HotKeyState::Pressed {
            if event.id == self.selector_hotkey.id() {
                if selector.is_visible() {
                    selector.hide();
                } else {
                    *last_selector_show = std::time::Instant::now();
                    selector.show(state.get_mode());
                }
            } else if event.id == self.notification_hotkey.id() {
                let new_val = !state.show_success_notification();
                state.set_show_success_notification(new_val);
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
            } else if event.id == self.pause_hotkey.id() {
                let new_paused = !state.is_paused();
                state.set_paused(new_paused);
                menu.pause_item.set_checked(new_paused);
                notifier::show_pause_notification(state, new_paused, "ショートカット");
                if !new_paused {
                    crate::tray::monitor::spawn_monitor_thread(Arc::clone(state));
                }
            } else if event.id == self.quit_hotkey.id() {
                *control_flow = ControlFlow::Exit;
            }
        }
    }
}

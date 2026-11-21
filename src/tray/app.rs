use std::sync::Arc;
use std::time::Instant;

use super::event;
use super::hotkey::HotkeyHandler;
use super::menu::TrayMenu;
use super::monitor::{init_clipboard, spawn_monitor_thread};
use super::selector::{SelectorWindow, init_selector};
use super::state::{AppEvent, AppState};

use anyhow::Result;
use arboard::Clipboard;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use tray_icon::menu::MenuEvent;

/// アプリケーション全体のコンテキストを管理する構造体
pub struct App {
    pub state: Arc<AppState>,
    pub menu: TrayMenu,
    pub selector: SelectorWindow,
    pub hotkey_handler: HotkeyHandler,
    pub clipboard: Clipboard,
    pub last_selector_show: Instant,
}

impl App {
    /// アプリケーションを初期化する。必要なコンポーネントをすべて生成し、監視を開始する。
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウを作成するためのイベントループのターゲット。
    /// * `proxy` - UIイベントを送信するためのプロキシ。
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `App` インスタンス。
    pub fn new(event_loop: &EventLoop<AppEvent>, proxy: EventLoopProxy<AppEvent>) -> Result<Self> {
        let state = Arc::new(AppState::new(proxy.clone()));
        let menu = TrayMenu::build(&state)?;
        let hotkey_handler = HotkeyHandler::new()?;
        let selector = init_selector(event_loop, proxy.clone())?;
        let clipboard = init_clipboard()?;

        hotkey_handler.start_event_listener(proxy);
        menu.refresh_history(&state)?;
        spawn_monitor_thread(Arc::clone(&state));

        Ok(Self {
            state,
            menu,
            selector,
            hotkey_handler,
            clipboard,
            last_selector_show: Instant::now(),
        })
    }

    /// メインループからのイベントを適切に振り分けて処理する。
    ///
    /// # Arguments
    /// * `event` - 受信したイベント。
    /// * `control_flow` - イベントループの制御フロー。
    pub fn handle_event(&mut self, event: Event<AppEvent>, control_flow: &mut ControlFlow) {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(app_event) => {
                self.handle_user_event(app_event, control_flow);
            }
            Event::WindowEvent {
                window_id, event, ..
            } if window_id == self.selector.id() => {
                event::handle_window_event(event, &self.selector, &self.last_selector_show);
            }
            _ => {
                if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                    event::handle_menu_event(
                        menu_event,
                        &self.menu,
                        &self.state,
                        &mut self.clipboard,
                        control_flow,
                    );
                }
            }
        }
    }

    /// `AppEvent`（カスタムユーザーイベント）を処理する。
    ///
    /// # Arguments
    /// * `event` - 処理対象の `AppEvent`。
    /// * `control_flow` - イベントループの制御フロー。
    fn handle_user_event(&mut self, event: AppEvent, control_flow: &mut ControlFlow) {
        match event {
            AppEvent::RequestModeChange(mode) => {
                self.selector.hide();
                event::update_refine(&self.state, &self.menu, &mut self.clipboard, mode);
            }
            AppEvent::HideSelector => {
                self.selector.hide();
            }
            AppEvent::RefreshHistory => {
                let _ = self.menu.refresh_history(&self.state);
            }
            AppEvent::Hotkey(hotkey_event) => {
                self.hotkey_handler.handle_event(
                    hotkey_event,
                    &self.state,
                    &self.menu,
                    &self.selector,
                    control_flow,
                    &mut self.last_selector_show,
                );
            }
        }
    }
}

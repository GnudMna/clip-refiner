use std::sync::Arc;
use std::time::Instant;

use super::event;
use super::hotkey::HotkeyHandler;
use super::menu::TrayMenu;
use super::monitor::spawn_monitor_thread;
use super::selector::{SelectorWindow, init_selector};
use super::state::{AppEvent, AppState};

use anyhow::Result;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use tray_icon::menu::MenuEvent;

/// アプリケーション全体のコンテキストとメインロジックを管理する構造体
///
/// 各コンポーネント（状態、メニュー、UI、ホットキー、ワーカー）を保持し、
/// イベントループからのメッセージを処理します。
pub struct App {
    /// アプリケーションの共有状態
    pub state: Arc<AppState>,
    /// システムトレイメニュー
    pub menu: TrayMenu,
    /// モード選択用のUIウィンドウ
    pub selector: SelectorWindow,
    /// グローバルホットキーの管理
    pub hotkey_handler: HotkeyHandler,
    /// クリップボード処理ワーカーへの送信チャネル
    pub clipboard_tx: std::sync::mpsc::Sender<super::worker::ClipboardCommand>,
    /// 最後にセレクタを表示した時刻（連打防止用）
    pub last_selector_show: Instant,
}

impl App {
    /// アプリケーションを初期化する
    ///
    /// 必要なコンポーネントをすべて生成し、ホットキーやクリップボードの監視を開始します。
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウを作成するためのイベントループ
    /// * `proxy` - UIスレッドへイベントを送信するためのプロキシ
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `App` インスタンス。失敗した場合はエラーを返します。
    pub fn new(event_loop: &EventLoop<AppEvent>, proxy: EventLoopProxy<AppEvent>) -> Result<Self> {
        let state = Arc::new(AppState::new(proxy.clone()));
        let menu = TrayMenu::build(&state)?;
        let hotkey_handler = HotkeyHandler::new()?;
        let selector = init_selector(event_loop, proxy.clone())?;
        let clipboard_tx = super::worker::spawn_clipboard_worker(Arc::clone(&state));

        hotkey_handler.start_event_listener(proxy);
        menu.refresh_history(&state)?;
        spawn_monitor_thread(Arc::clone(&state));

        Ok(Self {
            state,
            menu,
            selector,
            hotkey_handler,
            clipboard_tx,
            last_selector_show: Instant::now(),
        })
    }

    /// メインループからのイベントを受信し、適切に振り分けて処理する
    ///
    /// ユーザーイベント、ウィンドウイベント、メニューイベントなどを各ハンドラに委譲します。
    ///
    /// # Arguments
    /// * `event` - 受信したイベント
    /// * `control_flow` - イベントループの制御フロー
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
                        &self.clipboard_tx,
                        control_flow,
                    );
                }
            }
        }
    }

    /// アプリケーション独自のユーザーイベント (`AppEvent`) を処理する
    ///
    /// モード変更要求、ホットキー通知、履歴の更新などを処理します。
    ///
    /// # Arguments
    /// * `event` - 処理対象の `AppEvent`
    /// * `control_flow` - イベントループの制御フロー
    fn handle_user_event(&mut self, event: AppEvent, control_flow: &mut ControlFlow) {
        match event {
            AppEvent::RequestModeChange(mode) => {
                self.selector.hide();
                event::update_refine(&self.state, &self.menu, &self.clipboard_tx, mode);
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

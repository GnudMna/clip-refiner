use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::clipboard_monitor::bump_monitor_generation;
use super::event;
use super::hotkey::{HotkeyEventContext, HotkeyHandler};
use super::menu::TrayMenu;
use super::quick_selector::{QuickSelectorWindow, init_quick_selector};
use super::state::{AppEvent, AppState};
use super::text_selector::{TextSelectorWindow, init_text_selector};

use anyhow::Result;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use tray_icon::menu::MenuEvent;

// ======================================================================
// アプリケーション構造体
// ======================================================================
/// アプリケーション全体のコンテキストとメインロジックを管理する構造体
///
/// 各コンポーネント(状態、メニュー、UI、ホットキー、ワーカー)を保持し、
/// イベントループからのメッセージを処理する
pub struct App {
    /// アプリケーションの共有状態
    pub state: Arc<AppState>,
    /// システムトレイメニュー
    pub menu: TrayMenu,
    /// 加工モード選択用クイックセレクター
    pub quick_selector: QuickSelectorWindow,
    /// 登録文字列選択用の UI ウィンドウ
    pub text_selector: TextSelectorWindow,
    /// グローバルホットキーの管理
    pub hotkey_handler: HotkeyHandler,
    /// クリップボード処理ワーカーへの送信チャネル
    pub clipboard_tx: Sender<super::worker::ClipboardCommand>,
    /// 最後にクイックセレクターを表示した時刻(連打防止用)
    pub last_quick_selector_show: Instant,
    /// 最後にテキストセレクターを表示した時刻(連打防止用)
    pub last_text_selector_show: Instant,
}

// ======================================================================
// 初期化
// ======================================================================
impl App {
    /// アプリケーションを初期化する
    ///
    /// 必要なコンポーネントをすべて生成し、ホットキーやクリップボードの監視を開始する
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウを作成するためのイベントループ
    /// * `proxy` - UIスレッドへイベントを送信するためのプロキシ
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `App` インスタンス。失敗した場合はエラーを返す。
    pub fn new(event_loop: &EventLoop<AppEvent>, proxy: EventLoopProxy<AppEvent>) -> Result<Self> {
        let state = Arc::new(AppState::new(proxy.clone()));
        let menu = TrayMenu::build(&state)?;
        let hotkeys = state.with_config(|c| c.hotkeys.clone());
        let hotkey_handler = HotkeyHandler::new(&hotkeys)?;
        let quick_selector = init_quick_selector(event_loop, &proxy)?;
        let text_selector = init_text_selector(event_loop, &proxy)?;
        let clipboard_tx = super::worker::spawn_clipboard_worker(Arc::clone(&state));

        HotkeyHandler::start_event_listener(proxy);
        menu.refresh_history(&state)?;
        bump_monitor_generation(&state);

        Ok(Self {
            state,
            menu,
            quick_selector,
            text_selector,
            hotkey_handler,
            clipboard_tx,
            last_quick_selector_show: Instant::now(),
            last_text_selector_show: Instant::now(),
        })
    }
}

// ======================================================================
// イベント処理
// ======================================================================
impl App {
    /// メインループからのイベントを受信し、適切に振り分けて処理する
    ///
    /// ユーザーイベント、ウィンドウイベント、メニューイベントなどを各ハンドラに委譲する
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
            } if window_id == self.quick_selector.id() => {
                event::handle_window_event(
                    &event,
                    &self.quick_selector,
                    &self.last_quick_selector_show,
                );
            }
            Event::WindowEvent {
                window_id, event, ..
            } if window_id == self.text_selector.id() => {
                event::handle_window_event(
                    &event,
                    &self.text_selector,
                    &self.last_text_selector_show,
                );
            }
            _ => {
                if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                    event::handle_menu_event(
                        &menu_event,
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
    /// モード変更要求、ホットキー通知、履歴の更新などを処理する
    ///
    /// # Arguments
    /// * `event` - 処理対象の `AppEvent`
    /// * `control_flow` - イベントループの制御フロー
    fn handle_user_event(&mut self, event: AppEvent, control_flow: &mut ControlFlow) {
        match event {
            AppEvent::RequestModeChange(mode) => {
                self.quick_selector.hide();
                event::update_refine(&self.state, &self.menu, &self.clipboard_tx, mode);
            }
            AppEvent::HideQuickSelector => {
                self.quick_selector.hide();
            }
            AppEvent::HideTextSelector => {
                self.text_selector.hide();
            }
            AppEvent::RequestTextCopy(index) => {
                self.text_selector.hide();
                event::copy_registered_text(&self.state, &self.clipboard_tx, index);
            }
            AppEvent::RequestTextRegister => {
                let _ = self
                    .clipboard_tx
                    .send(super::worker::ClipboardCommand::RegisterFromClipboard);
            }
            AppEvent::RequestTextDelete(index) => {
                event::delete_registered_text(&self.state, &self.menu, &self.text_selector, index);
            }
            AppEvent::RequestFavoriteToggle(mode) => {
                event::toggle_favorite_mode(
                    &self.state,
                    &self.menu,
                    Some(&self.quick_selector),
                    mode,
                );
            }
            AppEvent::RequestFavoriteMove(mode, direction) => {
                event::move_favorite_mode(
                    &self.state,
                    &self.menu,
                    Some(&self.quick_selector),
                    mode,
                    direction,
                );
            }
            AppEvent::RefreshHistory => {
                let _ = self.menu.refresh_history(&self.state);
            }
            AppEvent::RefreshTexts => {
                event::refresh_texts_views(&self.state, &self.menu, &self.text_selector);
            }
            AppEvent::ReloadConfig => {
                event::reload_config_with_notification(
                    &self.state,
                    &self.menu,
                    &mut self.hotkey_handler,
                    &self.text_selector,
                );
            }
            AppEvent::Hotkey(hotkey_event) => {
                let mut ctx = HotkeyEventContext {
                    state: &self.state,
                    menu: &self.menu,
                    quick_selector: Some(&self.quick_selector),
                    text_selector: Some(&self.text_selector),
                    control_flow,
                    last_quick_selector_show: &mut self.last_quick_selector_show,
                    last_text_selector_show: &mut self.last_text_selector_show,
                    clipboard_tx: &self.clipboard_tx,
                };
                self.hotkey_handler.handle_event(hotkey_event, &mut ctx);
            }
        }
    }
}

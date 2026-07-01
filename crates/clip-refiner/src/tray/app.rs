use std::sync::Arc;

use super::clip_selector::{ClipSelectorWindow, init_clip_selector};
use super::clipboard_monitor::bump_monitor_generation;
use super::event;
use super::hotkey::{HotkeyEventContext, HotkeyHandler};
use super::menu::TrayMenu;
use super::ocr_capture::{OcrCaptureWindow, init_ocr_capture};
use super::quick_selector::{QuickSelectorWindow, init_quick_selector};
use super::state::{AppEvent, AppState};
use super::worker::ClipboardWorkerHandle;

use anyhow::Result;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use tray_icon::menu::MenuEvent;

use std::time::Instant;

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
    /// 登録クリップ選択用の UI ウィンドウ
    pub clip_selector: ClipSelectorWindow,
    /// 画面範囲選択 OCR 用オーバーレイ
    #[cfg(screen_ocr)]
    pub ocr_capture: OcrCaptureWindow,
    /// グローバルホットキーの管理
    pub hotkey_handler: HotkeyHandler,
    /// クリップボード処理ワーカー
    pub clipboard_worker: Arc<ClipboardWorkerHandle>,
    /// 最後にクイックセレクターを表示した時刻(連打防止用)
    pub last_quick_selector_show: Instant,
    /// 最後に登録クリップセレクターを表示した時刻(連打防止用)
    pub last_clip_selector_show: Instant,
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
        let state = Arc::new(AppState::new(proxy.clone())?);
        let menu = TrayMenu::build(&state)?;
        let (hotkeys, favorite_modes) =
            state.with_config(|c| (c.hotkeys.clone(), c.favorite_modes.clone()));
        let hotkey_handler = HotkeyHandler::new(&hotkeys, &favorite_modes)?;
        let quick_selector = init_quick_selector(event_loop, &proxy)?;
        let clip_selector = init_clip_selector(event_loop, &proxy)?;
        let clipboard_worker = ClipboardWorkerHandle::spawn(&state);
        #[cfg(screen_ocr)]
        let ocr_capture = init_ocr_capture(event_loop, &proxy, Arc::clone(&clipboard_worker))?;

        HotkeyHandler::start_event_listener(proxy);
        menu.refresh_history(&state)?;
        bump_monitor_generation(&state);

        Ok(Self {
            state,
            menu,
            quick_selector,
            clip_selector,
            #[cfg(screen_ocr)]
            ocr_capture,
            hotkey_handler,
            clipboard_worker,
            last_quick_selector_show: Instant::now(),
            last_clip_selector_show: Instant::now(),
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
            } if window_id == self.clip_selector.id() => {
                event::handle_window_event(
                    &event,
                    &self.clip_selector,
                    &self.last_clip_selector_show,
                );
            }
            #[cfg(all(screen_ocr, not(windows)))]
            Event::WindowEvent {
                window_id, event, ..
            } if self.ocr_capture.id() == window_id => {
                let _ = event;
            }
            _ => {
                if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                    event::handle_menu_event(
                        &menu_event,
                        &self.menu,
                        &self.state,
                        &self.clipboard_worker,
                        Some(&self.quick_selector),
                        control_flow,
                    );
                }
            }
        }
    }

    /// クリップボードワーカーの再起動・停止・復旧イベントを処理する
    fn handle_clipboard_worker_lifecycle(&mut self, event: AppEvent) {
        match event {
            AppEvent::RestartClipboardWorker => self.clipboard_worker.restart(),
            AppEvent::ClipboardWorkerStopped => {
                self.clipboard_worker.sync_menu_state(&self.menu);
                crate::platform::show_notification(
                    "クリップボードエラー",
                    super::worker::worker_stopped_notification_body(),
                );
            }
            AppEvent::ClipboardWorkerReady => {
                self.clipboard_worker.sync_menu_state(&self.menu);
                crate::platform::show_notification(
                    "クリップボード監視",
                    "クリップボード監視を再開しました",
                );
            }
            _ => {}
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
                event::update_refine(&self.state, &self.menu, &self.clipboard_worker, mode, None);
            }
            AppEvent::HideQuickSelector => {
                self.quick_selector.hide();
            }
            AppEvent::HideClipSelector => {
                self.clip_selector.hide();
            }
            AppEvent::HideOcrCapture => {
                #[cfg(screen_ocr)]
                self.ocr_capture.hide();
            }
            AppEvent::RequestClipCopy(index) => {
                self.clip_selector.hide();
                event::copy_registered_clip(&self.state, &self.clipboard_worker, index);
            }
            AppEvent::RequestClipRegister => {
                super::dispatch::send_clipboard_command(
                    &self.clipboard_worker,
                    super::worker::ClipboardCommand::RegisterClipFromClipboard,
                );
            }
            AppEvent::RequestClipDelete(index) => {
                event::delete_registered_clip(&self.state, &self.menu, &self.clip_selector, index);
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
                if let Err(err) = self.menu.refresh_history(&self.state) {
                    super::dispatch::log_menu_operation_error("履歴メニューの更新", err);
                }
            }
            AppEvent::RefreshClips => {
                event::refresh_clips_views(&self.state, &self.menu, &self.clip_selector);
            }
            AppEvent::ReloadConfig => {
                event::reload_config_with_notification(
                    &self.state,
                    &self.menu,
                    &mut self.hotkey_handler,
                    &self.clip_selector,
                );
            }
            AppEvent::RestartClipboardWorker
            | AppEvent::ClipboardWorkerStopped
            | AppEvent::ClipboardWorkerReady => self.handle_clipboard_worker_lifecycle(event),
            AppEvent::ReloadFavoriteHotkeys => {
                let (hotkeys, favorite_count) = self
                    .state
                    .with_config(|c| (c.hotkeys.clone(), c.favorite_modes.len()));
                if let Err(err) = self
                    .hotkey_handler
                    .reload_favorite_slots(&hotkeys, favorite_count)
                {
                    crate::log_warn!("お気に入りホットキーの再登録に失敗: {}", err);
                }
            }
            AppEvent::Hotkey(hotkey_event) => {
                let mut ctx = HotkeyEventContext {
                    state: &self.state,
                    menu: &self.menu,
                    quick_selector: Some(&self.quick_selector),
                    clip_selector: Some(&self.clip_selector),
                    #[cfg(screen_ocr)]
                    ocr_capture: Some(&self.ocr_capture),
                    control_flow,
                    last_quick_selector_show: &mut self.last_quick_selector_show,
                    last_clip_selector_show: &mut self.last_clip_selector_show,
                    clipboard_worker: &self.clipboard_worker,
                };
                self.hotkey_handler.handle_event(hotkey_event, &mut ctx);
            }
        }
    }
}

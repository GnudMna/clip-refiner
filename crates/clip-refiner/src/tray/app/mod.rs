//! アプリケーション全体のコンテキストとイベントループ連携

mod init;

use std::sync::Arc;
use std::time::Instant;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopWindowTarget};

use super::event;
use super::hotkey::{HotkeyHandler, HotkeyListenerStop};
use super::menu::TrayMenu;
use super::selector::SelectorUi;
use super::settings::SettingsWindow;
use super::state::{AppEvent, AppState};
use super::worker::ClipboardWorkerHandle;
use crate::ipc::OneshotIpcServer;

// ======================================================================
// アプリケーション構造体
// ======================================================================
/// 常駐アプリのコンポーネント束とイベントループ連携
///
/// イベントの振り分けは [`event`] モジュールへ委譲する
pub struct App {
    state: Arc<AppState>,
    menu: TrayMenu,
    selectors: SelectorUi,
    settings: SettingsWindow,
    hotkey_handler: HotkeyHandler,
    _hotkey_listener: HotkeyListenerStop,
    clipboard_worker: Arc<ClipboardWorkerHandle>,
    /// ワンショット CLI 向け IPC サーバー (E2E 時は `None`)
    _oneshot_ipc: Option<OneshotIpcServer>,
    last_quick_selector_show: Instant,
    last_clip_selector_show: Instant,
}

// ======================================================================
// イベント処理
// ======================================================================
impl App {
    /// メインループからのイベントを受信し、[`event`] モジュールへ委譲する
    ///
    /// # Arguments
    /// * `event` - 受信したイベント
    /// * `event_loop` - 遅延ウィンドウ生成用のイベントループ
    /// * `control_flow` - イベントループの制御フロー
    pub fn handle_event(
        &mut self,
        event: Event<AppEvent>,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        control_flow: &mut ControlFlow,
    ) {
        let mut ctx = event::AppEventContext {
            state: &self.state,
            menu: &self.menu,
            selectors: &self.selectors,
            settings: &self.settings,
            hotkey_handler: &mut self.hotkey_handler,
            clipboard_worker: &self.clipboard_worker,
            event_loop,
            control_flow,
            last_quick_selector_show: &mut self.last_quick_selector_show,
            last_clip_selector_show: &mut self.last_clip_selector_show,
        };
        event::handle_loop_event(event, &mut ctx);
    }
}

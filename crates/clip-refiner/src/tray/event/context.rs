use std::sync::Arc;
use std::time::Instant;

use super::super::hotkey::HotkeyHandler;
use super::super::menu::TrayMenu;
use super::super::selector_ui::SelectorUi;
use super::super::state::AppState;
use super::super::worker::ClipboardWorkerHandle;

use tao::event_loop::ControlFlow;

// ======================================================================
// イベント処理コンテキスト
// ======================================================================
/// `AppEvent` およびイベントループからのイベント処理に必要な UI 状態の参照
pub(crate) struct AppEventContext<'a> {
    /// アプリケーションの共有状態
    pub state: &'a Arc<AppState>,
    /// システムトレイメニュー
    pub menu: &'a TrayMenu,
    /// セレクター系 UI
    pub selectors: &'a SelectorUi,
    /// クイックセレクターを最後に表示した時刻 (更新用)
    pub last_quick_selector_show: &'a mut Instant,
    /// 登録クリップセレクターを最後に表示した時刻 (更新用)
    pub last_clip_selector_show: &'a mut Instant,
    /// グローバルホットキー管理
    pub hotkey_handler: &'a mut HotkeyHandler,
    /// クリップボードワーカー
    pub clipboard_worker: &'a Arc<ClipboardWorkerHandle>,
    /// イベントループの制御フロー
    pub control_flow: &'a mut ControlFlow,
}

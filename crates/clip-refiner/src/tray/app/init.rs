use std::sync::Arc;
use std::time::Instant;

use super::super::clipboard_monitor::bump_monitor_generation;
use super::super::hotkey::{HotkeyHandler, notify_registration_failures};
use super::super::menu::TrayMenu;
use super::super::paste_watcher::PasteWatcher;
use super::super::selector_ui::SelectorUi;
use super::super::state::{AppEvent, AppState};
use super::super::worker::ClipboardWorkerHandle;
use super::App;

use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopProxy};

// ======================================================================
// 初期化
// ======================================================================
impl App {
    /// 必要なコンポーネントをすべて生成し、ホットキーやクリップボードの監視を開始する
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウを作成するためのイベントループ
    /// * `proxy` - UI スレッドへイベントを送信するためのプロキシ
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `App` インスタンス。失敗した場合は `Err`
    ///
    /// # Errors
    /// 状態・メニュー・ホットキー・セレクター UI のいずれかの初期化に失敗した場合
    pub fn new(event_loop: &EventLoop<AppEvent>, proxy: &EventLoopProxy<AppEvent>) -> Result<Self> {
        let (state, config_fallback) = AppState::new(proxy.clone())?;
        let state = Arc::new(state);
        if let Some(ref fallback) = config_fallback {
            crate::log_warn!("{}", fallback.user_message());
            crate::platform::show_notification("設定の読み込み", &fallback.user_message());
        }
        let menu = TrayMenu::build(&state)?;
        let (hotkeys, favorite_modes) =
            state.with_config(|c| (c.hotkeys.clone(), c.favorite_modes.clone()));
        let (hotkey_handler, hotkey_failures) = HotkeyHandler::new(&hotkeys, &favorite_modes)?;
        notify_registration_failures(&hotkey_failures);
        let clipboard_worker = ClipboardWorkerHandle::spawn(&state);
        let selectors = SelectorUi::new(event_loop, proxy, Arc::clone(&clipboard_worker))?;
        let paste_watcher = PasteWatcher::start(
            proxy.clone(),
            Arc::clone(&state),
            Arc::clone(&clipboard_worker),
        );
        let hotkey_listener = HotkeyHandler::start_event_listener(proxy.clone());
        menu.refresh_history(&state)?;
        menu.refresh_continuous_record(&state)?;
        bump_monitor_generation(&state);

        Ok(Self {
            state,
            menu,
            selectors,
            hotkey_handler,
            _hotkey_listener: hotkey_listener,
            clipboard_worker,
            _paste_watcher: paste_watcher,
            last_quick_selector_show: Instant::now(),
            last_clip_selector_show: Instant::now(),
        })
    }
}

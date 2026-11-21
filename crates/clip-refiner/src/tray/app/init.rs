use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tao::event_loop::EventLoopProxy;

use super::super::clipboard_monitor::{bump_monitor_generation, bump_monitor_generation_unchecked};
use super::super::hotkey::{HotkeyHandler, notify_registration_failures};
use super::super::menu::TrayMenu;
use super::super::settings::init_settings_window;
use super::super::state::{AppEvent, AppState};
use super::super::worker::ClipboardWorkerHandle;
use super::App;
use crate::ipc::OneshotIpcServer;
use crate::tray::selector::SelectorUi;
use crate::user_error::oneshot_ipc_start_failure;

// ======================================================================
// 初期化
// ======================================================================
impl App {
    /// 必要なコンポーネントをすべて生成し、ホットキーやクリップボードの監視を開始する
    ///
    /// # Arguments
    /// * `proxy` - UI スレッドへイベントを送信するためのプロキシ
    ///
    /// # Returns
    /// * `Result<Self>` - 初期化された `App` インスタンス。失敗した場合は `Err`
    ///
    /// # Errors
    /// 状態・メニュー・ホットキーのいずれかの初期化に失敗した場合
    pub fn new(proxy: &EventLoopProxy<AppEvent>) -> Result<Self> {
        let (state, config_fallback, unknown_keys, unsupported_version_kept) =
            AppState::new(proxy.clone())?;
        let state = Arc::new(state);
        if let Some(ref fallback) = config_fallback {
            crate::log_warn!("{}", fallback.user_message());
            crate::platform::show_notification("設定の読み込み", &fallback.user_message());
        }
        if !unknown_keys.is_empty() {
            let message = crate::config::format_unknown_config_keys_message(&unknown_keys);
            crate::log_warn!("{message}");
            crate::platform::show_notification("設定の読み込み", &message);
        }
        if unsupported_version_kept {
            let message = crate::config::format_unsupported_config_version_message();
            crate::log_warn!("{message}");
            crate::platform::show_notification("設定の読み込み", message);
        }
        let menu = TrayMenu::build(&state)?;
        let (hotkeys, favorite_modes) =
            state.with_config(|c| (c.hotkeys.clone(), c.favorite_modes.clone()));
        let (hotkey_handler, hotkey_failures) = HotkeyHandler::new(&hotkeys, &favorite_modes)?;
        notify_registration_failures(&hotkey_failures);
        let clipboard_worker = ClipboardWorkerHandle::spawn(&state);
        let oneshot_ipc = if crate::tray::e2e::is_active() {
            None
        } else {
            match OneshotIpcServer::start(Arc::clone(&clipboard_worker)) {
                Ok(server) => Some(server),
                Err(err) => {
                    oneshot_ipc_start_failure().notify_with_debug(&err);
                    None
                }
            }
        };
        let selectors = SelectorUi::new(proxy, Arc::clone(&clipboard_worker));
        let settings = init_settings_window(proxy, &state);
        let hotkey_listener = HotkeyHandler::start_event_listener(proxy.clone());
        menu.refresh_history(&state)?;
        menu.refresh_continuous_record(&state)?;
        if state.with_config(|c| c.is_paused) {
            if state.with_config(|c| c.continuous_record_enabled) {
                bump_monitor_generation_unchecked(&state);
            }
        } else {
            bump_monitor_generation(&state);
        }
        if !crate::tray::e2e::is_active() {
            crate::platform::webview2::notify_missing_runtime_on_startup();
        }

        Ok(Self {
            state,
            menu,
            selectors,
            settings,
            hotkey_handler,
            _hotkey_listener: hotkey_listener,
            clipboard_worker,
            _oneshot_ipc: oneshot_ipc,
            last_quick_selector_show: Instant::now(),
            last_clip_selector_show: Instant::now(),
        })
    }
}

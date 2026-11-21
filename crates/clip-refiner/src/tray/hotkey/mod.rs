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

#[cfg(test)]
mod tests;

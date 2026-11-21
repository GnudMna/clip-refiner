//! システムトレイアイコン、各種メニュー、およびホットキー監視を管理するモジュール
//!
//! アプリケーションの常駐実行に関連する UI と背後のイベントループ制御を担当する

pub mod app;
mod clipboard_change;
pub(crate) mod clipboard_monitor;
mod dispatch;
pub mod event;
pub mod history;
pub mod hotkey;
pub mod menu;
mod notify;
#[cfg(windows)]
pub mod ocr_capture;
pub mod quick_selector;
mod runner;
mod selector_window;
pub mod state;
pub mod text_selector;
pub mod worker;

pub use runner::run_loop;

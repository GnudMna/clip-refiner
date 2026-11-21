//! システムトレイアイコン、各種メニュー、およびホットキー監視を管理するモジュール
//!
//! アプリケーションの常駐実行に関連する UI と背後のイベントループ制御を担当する

pub mod app;
pub(crate) mod clipboard_monitor;
mod dispatch;
pub mod event;
mod features;
pub mod history_store;
pub mod hotkey;
pub mod menu;
mod notify;
mod ocr;
mod paste_watcher;
mod runner;
pub mod selector;
pub mod settings;
pub mod state;
pub mod worker;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use paste_watcher::try_paste_continuous_record_front;

pub use runner::run_loop;

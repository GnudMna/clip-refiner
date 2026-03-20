//! システムトレイアイコン、各種メニュー、およびホットキー監視を管理するモジュール
//!
//! アプリケーションの常駐実行に関連する UI と背後のイベントループ制御を担当します。

pub mod app;
pub mod event;
pub mod hotkey;
pub mod menu;
pub mod monitor;
pub mod notifier;
mod runner;
pub mod selector;
pub mod state;
pub mod worker;

pub use runner::run_loop;

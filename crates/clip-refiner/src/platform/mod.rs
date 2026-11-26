//! デスクトップ常駐向けの OS 連携 (通知・自動起動・画面キャプチャ・OCR など)
//!
//! 加工ロジック向けのネイティブ API (DPAPI・クリップボード画像) は `clip-refiner-core` の `native/` を参照

pub(crate) mod autostart;
pub(crate) mod bundled_docs;
pub(crate) mod ocr;
pub(crate) mod ocr_overlay;
pub(crate) mod screen_capture;
pub(crate) mod shell;
pub(crate) mod single_instance;
pub(crate) mod webview2;

mod notify;

pub use notify::{init_notifications, show_notification};

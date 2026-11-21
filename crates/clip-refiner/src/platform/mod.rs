//! OS 連携 (通知・自動起動・画面キャプチャ・OCR など)

pub(crate) mod autostart;
pub(crate) mod ocr;
pub(crate) mod ocr_overlay;
pub(crate) mod screen_capture;

mod notify;

pub use notify::{init_notifications, show_notification};

//! OS プラットフォーム連携 (デスクトップ通知・画面キャプチャ・OCR など)

mod notify;
pub(crate) mod ocr;
pub(crate) mod ocr_overlay;
pub(crate) mod screen_capture;

pub(crate) mod clipboard_image;

#[cfg(windows)]
mod notify_windows;

#[cfg(not(windows))]
pub use notify::show_notification;
#[cfg(windows)]
pub use notify_windows::{init_notifications, show_notification};

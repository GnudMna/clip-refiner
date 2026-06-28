//! OS プラットフォーム連携 (デスクトップ通知など)

mod notify;

pub(crate) mod clipboard_image;

#[cfg(windows)]
mod notify_windows;

#[cfg(not(windows))]
pub use notify::show_notification;
#[cfg(windows)]
pub use notify_windows::{init_notifications, show_notification};

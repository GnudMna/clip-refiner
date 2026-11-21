//! 加工完了・自動変換の切替などアプリ固有の通知メッセージ組み立て

mod message;

pub use message::{
    NotificationCategory, show_auto_refine_notification, show_category_notification,
    show_exclusive_mode_notification, show_image_process_notification, show_process_notification,
};

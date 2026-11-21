//! 加工完了・一時停止などアプリ固有の通知メッセージ組み立て

mod message;

pub use message::{
    NotificationCategory, show_category_notification, show_image_process_notification,
    show_pause_notification, show_process_notification,
};

//! トレイメニューの機能ドメイン別実装
//!
//! 各サブモジュールはメニュー構築 (`menu.rs`) とクリック処理 (`event.rs`) を担う
//! [`super::menu`] はトレイ全体の骨格 ([`TrayMenu`](super::menu::TrayMenu) 型・組み立て・同期)、
//! [`super::event`] はループ制御とルーティングを担当する

pub mod app_control;
pub mod clips;
pub mod history;
pub mod monitor;
pub mod notification;
pub mod record;
pub mod refine;
pub mod update;

pub(crate) use app_control::handle_app_control;
pub(crate) use clips::{
    copy_registered_clip, delete_registered_clip, handle_clips_event, refresh_clips_views,
};
pub(crate) use history::handle_history_event;
pub(crate) use monitor::handle_monitor_event;
pub(crate) use notification::handle_notification_event;
pub(crate) use record::{
    handle_continuous_record_event, toggle_continuous_record, undo_last_continuous_record,
};
pub(crate) use refine::{
    handle_favorites_event, handle_refine_mode_event, move_favorite_mode, toggle_favorite_mode,
};
pub(crate) use update::{
    handle_update_apply_finished, handle_update_check_finished, start_update_apply,
    start_update_check,
};

pub use monitor::update_monitor_mode;
pub use refine::update_refine;

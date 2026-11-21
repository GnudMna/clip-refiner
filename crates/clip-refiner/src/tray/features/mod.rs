//! トレイメニューの機能ドメイン別実装
//!
//! 各サブモジュールはメニュー構築 (`menu.rs`) とクリック処理 (`event.rs`) を担う
//! [`super::menu`] はトレイ全体の骨格 ([`TrayMenu`](super::menu::TrayMenu) 型・組み立て・同期)、
//! [`super::event`] はループ制御とルーティングを担当する

pub mod app_control;
pub mod clips;
pub mod history;
pub mod monitor;
pub mod record;
pub mod refine;
pub mod regex;
pub mod update;

pub(crate) use app_control::handle_app_control;
pub(crate) use clips::{
    STALE_CLIP_MESSAGE, StaleClip, copy_registered_clip, delete_registered_clip,
    handle_clips_event, move_registered_clip, refresh_clips_views, registered_clip_for_edit,
    reset_registered_clip_counter, set_registered_clip_template, show_registered_clip_full_text,
    update_registered_clip,
};
pub(crate) use history::handle_history_event;
pub(crate) use record::{
    handle_continuous_record_event, set_continuous_record_enabled, toggle_continuous_record,
    undo_last_continuous_record,
};
pub(crate) use refine::{
    apply_refine_once, handle_favorites_event, handle_refine_mode_event, move_favorite_mode,
    select_auto_refine_mode, set_auto_refine_enabled, toggle_favorite_mode,
};
pub(crate) use regex::handle_regex_preset_event;
pub(crate) use update::{
    handle_update_apply_finished, handle_update_check_finished, start_update_apply,
    start_update_check,
};

pub use monitor::update_monitor_mode;
pub use refine::update_refine;

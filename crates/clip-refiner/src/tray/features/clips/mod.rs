mod event;
mod menu;

pub(crate) use event::{
    copy_registered_clip, delete_registered_clip, handle_clips_event, move_registered_clip,
    refresh_clips_views, set_registered_clip_template,
};

mod event;
mod menu;

pub(crate) use event::{
    handle_continuous_record_event, set_continuous_record_enabled, toggle_continuous_record,
    undo_last_continuous_record,
};

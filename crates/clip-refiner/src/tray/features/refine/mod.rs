mod event;
mod favorites;
mod menu;

#[cfg(test)]
pub use event::update_refine;
pub(crate) use event::{
    apply_refine_once, handle_refine_mode_event, select_auto_refine_mode, set_auto_refine_enabled,
    set_refine_mode,
};
pub(crate) use favorites::{handle_favorites_event, move_favorite_mode, toggle_favorite_mode};

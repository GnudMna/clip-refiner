mod command;
mod feedback;
mod patch;
mod refresh;

pub(crate) use command::{SettingsWindowCommand, handle_settings_window_command};
pub(crate) use feedback::{SettingsApplyFeedback, should_notify_apply_error_via_desktop};
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use patch::apply_settings_patch_without_ui;
pub(crate) use refresh::{
    refresh_settings_clips, refresh_settings_favorites_and_shortcuts, refresh_settings_window,
    refresh_settings_window_after_external_reload,
};

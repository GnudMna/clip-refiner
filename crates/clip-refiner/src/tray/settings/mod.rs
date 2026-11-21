//! 設定ウィンドウ (`WebView` ベース)

mod apply;
mod geometry;
mod ipc;
mod model;
mod regex_preview;
mod snapshot;
#[cfg(test)]
mod webview_contract_tests;
mod window;

pub use window::{SettingsWindow, init_settings_window};

pub(crate) use apply::{
    refresh_settings_clips, refresh_settings_favorites_and_shortcuts, refresh_settings_window,
    refresh_settings_window_after_external_reload,
};

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) mod test_support {
    pub(crate) use super::apply::apply_settings_patch_without_ui;
    pub(crate) use super::ipc::{
        SettingsIpcAction, enqueue_settings_patch, enqueue_settings_patch_json,
        parse_settings_ipc_message,
    };
    pub(crate) use super::model::SettingsPatch;
}

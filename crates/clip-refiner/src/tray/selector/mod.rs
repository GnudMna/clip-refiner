//! セレクター系 `WebView` UI

mod clip;
mod quick;
mod retention;
mod ui;
mod view_model;
mod webview_policy;
mod webview_script;
mod window;

pub use clip::ClipSelectorWindow;
pub use quick::QuickSelectorWindow;
pub(crate) use retention::{apply_idle_control_flow, hide_with_retention, sync_selector_retention};
pub use ui::SelectorUi;
pub(crate) use view_model::{
    clip_selector_json, clip_selector_value, quick_selector_current_mode, quick_selector_json,
    quick_selector_value,
};
#[cfg(test)]
pub(crate) use webview_policy::assert_inline_scripts_allowed_by_csp;
pub(crate) use webview_policy::{apply_webview_csp, inline_script_block};
pub(crate) use webview_script::{json_parse_call_script, settings_load_script};
pub(crate) use window::{
    WebSelectorWindow, WebViewWindowOptions, WindowVerticalAlign, position_window_on_monitor,
};

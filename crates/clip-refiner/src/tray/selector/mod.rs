//! セレクター系 `WebView` UI

mod clip;
mod quick;
mod retention;
mod ui;
mod webview_policy;
mod webview_script;
mod window;

pub use clip::ClipSelectorWindow;
pub use quick::QuickSelectorWindow;
pub(crate) use retention::{apply_idle_control_flow, hide_with_retention, sync_selector_retention};
pub use ui::SelectorUi;
pub(crate) use webview_policy::apply_webview_csp;
pub(crate) use webview_script::{json_parse_call_script, settings_load_script};
pub(crate) use window::{
    WebSelectorWindow, WebViewWindowOptions, WindowVerticalAlign, position_window_on_monitor,
};

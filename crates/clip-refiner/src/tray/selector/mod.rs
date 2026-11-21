//! セレクター系 `WebView` UI

mod clip;
mod quick;
mod ui;
mod webview_script;
mod window;

pub use clip::ClipSelectorWindow;
pub use quick::QuickSelectorWindow;
pub use ui::SelectorUi;
pub(crate) use webview_script::{json_parse_call_script, settings_load_script};
pub(crate) use window::{
    WebSelectorWindow, WebViewWindowOptions, WindowVerticalAlign, position_window_on_monitor,
};

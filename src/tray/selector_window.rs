//! コマンドパレット風 `WebView` セレクターの共通基盤
//!
//! クイックセレクターと登録クリップセレクターで共有するウィンドウ生成・表示制御を提供する

use std::sync::Arc;

use super::state::AppEvent;

use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{WebContext, WebView, WebViewBuilder};

// ======================================================================
// HTML 組み立て
// ======================================================================
/// セレクター HTML 内の CSS `@import` をインライン展開する
pub(crate) fn embed_selector_css(html: &str) -> String {
    let css = include_str!("../ui/selector.css");
    html.replace("@import url(\"selector.css\");", css)
}

// ======================================================================
// WebView セレクターウィンドウ
// ======================================================================
/// `wry` ベースの透明セレクターウィンドウ
pub(crate) struct WebSelectorWindow {
    /// `WebView` インスタンス
    webview: WebView,
    /// 描画先ウィンドウ
    window: Arc<Window>,
    /// `WebView` コンテキスト
    _context: WebContext,
}

impl WebSelectorWindow {
    /// 非表示のベースウィンドウと `WebView` を生成する
    ///
    /// # Arguments
    /// * `window` - ベースとなる `tao` ウィンドウ
    /// * `webview_data_dir_suffix` - `WebView2` データディレクトリ名のサフィックス
    /// * `html` - 初期表示 HTML
    /// * `ipc_handler` - JavaScript からの IPC コールバック
    pub fn build(
        window: Window,
        webview_data_dir_suffix: &str,
        html: String,
        ipc_handler: impl Fn(wry::http::Request<String>) + 'static,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let data_dir = std::env::temp_dir().join(webview_data_dir_suffix);
        let mut context = WebContext::new(Some(data_dir));

        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(html)
            .with_ipc_handler(ipc_handler)
            .build(&window)?;

        Ok(Self {
            webview,
            window,
            _context: context,
        })
    }

    /// ウィンドウを表示し、JavaScript を実行する
    pub fn show_with_script(&self, script: &str) {
        self.window.set_visible(true);
        self.window.set_focus();
        self.evaluate_script(script);
    }

    /// JavaScript を実行する
    pub fn evaluate_script(&self, script: &str) {
        if let Err(err) = self.webview.evaluate_script(script) {
            crate::log_warn!("セレクターの JavaScript 実行に失敗: {:?}", err);
        }
    }

    /// ウィンドウを非表示にする
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// ウィンドウが表示中かどうか
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// ウィンドウ ID を返す
    pub fn id(&self) -> WindowId {
        self.window.id()
    }
}

// ======================================================================
// ウィンドウ生成
// ======================================================================
/// 画面中央付近に配置する非表示セレクターウィンドウを生成する
pub(crate) fn build_hidden_selector_window(
    event_loop: &EventLoopWindowTarget<AppEvent>,
    title: &str,
    width: f64,
    height: f64,
) -> anyhow::Result<Window> {
    let window = WindowBuilder::new()
        .with_title(title)
        .with_always_on_top(true)
        .with_decorations(false)
        .with_transparent(true)
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(tao::dpi::LogicalSize::new(width, height))
        .build(event_loop)?;

    if let Some(monitor) = window.current_monitor() {
        let screen_size = monitor.size();
        let window_size = window.outer_size();
        let x = (screen_size.width.cast_signed() - window_size.width.cast_signed()) / 2;
        let y = (screen_size.height.cast_signed() - window_size.height.cast_signed()) / 3;
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }

    Ok(window)
}

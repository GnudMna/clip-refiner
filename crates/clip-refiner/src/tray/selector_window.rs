//! コマンドパレット風 `WebView` セレクターの共通基盤
//!
//! クイックセレクターと登録クリップセレクターで共有するウィンドウ生成・表示制御を提供する

use std::sync::Arc;

use super::state::AppEvent;
use super::webview_script;

use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{WebContext, WebView, WebViewBuilder};

// ======================================================================
// ウィンドウ配置
// ======================================================================
/// モニター上でのウィンドウ垂直位置
#[derive(Clone, Copy)]
pub(crate) enum WindowVerticalAlign {
    /// 画面中央
    Center,
    /// 上寄り (セレクター向け)
    UpperThird,
}

/// 生成済みウィンドウを現在のモニター中央付近へ配置する
pub(crate) fn position_window_on_monitor(window: &Window, align: WindowVerticalAlign) {
    let Some(monitor) = window.current_monitor() else {
        return;
    };

    let screen_size = monitor.size();
    let window_size = window.outer_size();
    let x = (screen_size.width.cast_signed() - window_size.width.cast_signed()) / 2;
    let y = match align {
        WindowVerticalAlign::Center => {
            (screen_size.height.cast_signed() - window_size.height.cast_signed()) / 2
        }
        WindowVerticalAlign::UpperThird => {
            (screen_size.height.cast_signed() - window_size.height.cast_signed()) / 3
        }
    };
    window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
}

// ======================================================================
// HTML 組み立て
// ======================================================================
const SELECTOR_CSS_IMPORT: &str = r#"@import url("selector.css");"#;
const CLIP_SELECTOR_CSS_IMPORT: &str = r#"@import url("clip_selector.css");"#;
const SELECTOR_COMMON_JS_TAG: &str = r#"<script src="selector-common.js"></script>"#;
const QUICK_SELECTOR_JS_TAG: &str = r#"<script src="quick_selector.js"></script>"#;
const CLIP_SELECTOR_JS_TAG: &str = r#"<script src="clip_selector.js"></script>"#;

/// セレクター HTML 内の共通アセット (CSS / JS) をインライン展開する
pub(crate) fn embed_selector_assets(html: &str) -> String {
    let css = include_str!("../ui/selector.css");
    let common_js = include_str!("../ui/selector-common.js");

    html.replace(SELECTOR_CSS_IMPORT, css).replace(
        SELECTOR_COMMON_JS_TAG,
        &format!("<script>\n{common_js}\n</script>"),
    )
}

/// クイックセレクター用 HTML 内のアセットをインライン展開する
pub(crate) fn embed_quick_selector_assets(html: &str) -> String {
    let quick_js = include_str!("../ui/quick_selector.js");
    embed_selector_assets(html).replace(
        QUICK_SELECTOR_JS_TAG,
        &format!("<script>\n{quick_js}\n</script>"),
    )
}

/// 登録クリップセレクター用 HTML 内のアセットをインライン展開する
pub(crate) fn embed_clip_selector_assets(html: &str) -> String {
    let clip_css = include_str!("../ui/clip_selector.css");
    let clip_js = include_str!("../ui/clip_selector.js");
    embed_selector_assets(html)
        .replace(CLIP_SELECTOR_CSS_IMPORT, clip_css)
        .replace(
            CLIP_SELECTOR_JS_TAG,
            &format!("<script>\n{clip_js}\n</script>"),
        )
}

// ======================================================================
// WebView ウィンドウ
// ======================================================================
/// `WebView` 構築時の表示オプション
#[derive(Clone, Copy)]
pub(crate) struct WebViewWindowOptions {
    /// 透明ウィンドウとして構築するか
    pub transparent: bool,
}

impl Default for WebViewWindowOptions {
    fn default() -> Self {
        Self { transparent: true }
    }
}

impl WebViewWindowOptions {
    /// 通常の不透明ウィンドウ (設定画面など)
    pub const OPAQUE: Self = Self { transparent: false };
}

/// `wry` ベースの `WebView` ウィンドウ
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
    /// * `options` - 透明化などの表示オプション
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 成功時は `WebSelectorWindow`、失敗時はエラー
    pub fn build(
        window: Window,
        webview_data_dir_suffix: &str,
        html: String,
        ipc_handler: impl Fn(wry::http::Request<String>) + 'static,
        options: WebViewWindowOptions,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let data_dir = std::env::temp_dir().join(webview_data_dir_suffix);
        let mut context = WebContext::new(Some(data_dir));

        let mut builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_html(html)
            .with_ipc_handler(ipc_handler);
        if options.transparent {
            builder = builder
                .with_transparent(true)
                .with_background_color((0, 0, 0, 0));
        }
        let webview = builder.build(&window)?;

        Ok(Self {
            webview,
            window,
            _context: context,
        })
    }

    /// ウィンドウを表示し、JavaScript を実行する
    ///
    /// # Arguments
    /// * `script` - 表示直後に実行する JavaScript
    pub fn show_with_script(&self, script: &str) {
        self.window.set_visible(true);
        self.window.set_focus();
        self.evaluate_script(script);
    }

    /// JavaScript を実行する
    ///
    /// # Arguments
    /// * `script` - 実行する JavaScript
    pub fn evaluate_script(&self, script: &str) {
        webview_script::evaluate_script(&self.webview, "WebView", script);
    }

    /// ウィンドウを非表示にする
    ///
    /// # Arguments
    /// * `self` - この `WebSelectorWindow`
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// ウィンドウが表示中かどうか
    ///
    /// # Arguments
    /// * `self` - この `WebSelectorWindow`
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// ウィンドウ ID を返す
    ///
    /// # Arguments
    /// * `self` - この `WebSelectorWindow`
    ///
    /// # Returns
    /// * `WindowId` - ウィンドウ ID
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// 描画先ウィンドウへの参照を返す
    pub(crate) fn window(&self) -> &Window {
        self.window.as_ref()
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

    position_window_on_monitor(&window, WindowVerticalAlign::UpperThird);

    Ok(window)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 共通 CSS / JS がインライン展開されるようにすること
    #[test]
    fn embed_selector_assets_inlines_common_files() {
        let html = embed_selector_assets(
            r#"<style>@import url("selector.css");</style><script src="selector-common.js"></script>"#,
        );
        assert!(!html.contains(SELECTOR_CSS_IMPORT));
        assert!(!html.contains(SELECTOR_COMMON_JS_TAG));
        assert!(html.contains("window.SelectorCommon"));
        assert!(html.contains("--bg-color"));
    }

    /// クイックセレクター専用 JS がインライン展開されるようにすること
    #[test]
    fn embed_quick_selector_assets_inlines_quick_js() {
        let html = embed_quick_selector_assets(
            r#"<style>@import url("selector.css");</style><script src="selector-common.js"></script><script src="quick_selector.js"></script>"#,
        );
        assert!(!html.contains(QUICK_SELECTOR_JS_TAG));
        assert!(html.contains("window.refreshModes"));
        assert!(html.contains("FAVORITES_CATEGORY"));
    }

    /// 登録クリップセレクター専用 CSS / JS がインライン展開されるようにすること
    #[test]
    fn embed_clip_selector_assets_inlines_clip_assets() {
        let html = embed_clip_selector_assets(
            r#"<style>@import url("selector.css");@import url("clip_selector.css");</style><script src="selector-common.js"></script><script src="clip_selector.js"></script>"#,
        );
        assert!(!html.contains(CLIP_SELECTOR_CSS_IMPORT));
        assert!(!html.contains(CLIP_SELECTOR_JS_TAG));
        assert!(html.contains("#image-hover-preview"));
        assert!(html.contains("window.refreshItems"));
    }
}

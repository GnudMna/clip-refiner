//! コマンドパレット風 `WebView` セレクターの共通基盤
//!
//! クイックセレクターと登録クリップセレクターで共有するウィンドウ生成・表示制御を提供する

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{NewWindowResponse, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows};

use super::webview_policy::{
    apply_webview_csp, inline_script_block, is_allowed_webview_navigation,
    preview_denied_webview_url,
};
use super::webview_script;
use crate::tray::state::AppEvent;

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

/// 生成済みウィンドウを現在のモニターの水平中央へ配置する (垂直位置は `align` に従う)
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
const QUICK_SELECTOR_JS_TAG: &str = r#"<script src="quick_selector.js"></script>"#;
const CLIP_SELECTOR_JS_TAG: &str = r#"<script src="clip_selector.js"></script>"#;

/// セレクター HTML 内の共通 CSS をインライン展開する
pub(crate) fn embed_selector_assets(html: &str) -> String {
    let css = include_str!("../../ui/selector/selector.css");
    html.replace(SELECTOR_CSS_IMPORT, css)
}

/// クイックセレクター用 HTML 内のアセットをインライン展開し、CSP を適用する
///
/// JS は `npm run build:ui` で生成した `quick_selector.bundle.js` を埋め込み、
/// そのハッシュだけを `script-src` で許可する
pub(crate) fn embed_quick_selector_assets(html: &str) -> String {
    let js = include_str!("../../ui/selector/quick_selector.bundle.js");
    let html = embed_selector_assets(html).replace(QUICK_SELECTOR_JS_TAG, &inline_script_block(js));
    apply_webview_csp(&html, js)
}

/// 登録クリップセレクター用 HTML 内のアセットをインライン展開し、CSP を適用する
///
/// JS は `npm run build:ui` で生成した `clip_selector.bundle.js` を埋め込み、
/// そのハッシュだけを `script-src` で許可する
pub(crate) fn embed_clip_selector_assets(html: &str) -> String {
    let clip_css = include_str!("../../ui/selector/clip_selector.css");
    let js = include_str!("../../ui/selector/clip_selector.bundle.js");
    let html = embed_selector_assets(html)
        .replace(CLIP_SELECTOR_CSS_IMPORT, clip_css)
        .replace(CLIP_SELECTOR_JS_TAG, &inline_script_block(js));
    apply_webview_csp(&html, js)
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
    /// 渡された非表示ウィンドウ上に `WebView` を生成する
    ///
    /// 直前プロセスの `WebView2` ユーザーデータロックで失敗した場合に備え、
    /// 同一ディレクトリ再試行のあとプロセス固有ディレクトリへフォールバックする
    /// 外部 URL へのページ遷移・`window.open`・ダウンロードは拒否し、
    /// 右クリックの既定コンテキストメニューも無効化する
    ///
    /// # Arguments
    /// * `window` - ホストとなる `tao` ウィンドウ
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
        html: &str,
        ipc_handler: impl Fn(wry::http::Request<String>) + Send + Sync + 'static,
        options: WebViewWindowOptions,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let data_dir_suffix = crate::tray::e2e::webview_data_dir_suffix(webview_data_dir_suffix);
        let base_dir = std::env::temp_dir().join(&data_dir_suffix);
        let fallback_dir = base_dir.join(format!("pid-{}", std::process::id()));
        let ipc_handler: Arc<dyn Fn(wry::http::Request<String>) + Send + Sync + 'static> =
            Arc::new(ipc_handler);

        let mut last_error = None;
        for (attempt, data_dir) in [&base_dir, &base_dir, &fallback_dir]
            .into_iter()
            .enumerate()
        {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(120));
            }
            match Self::build_once(Arc::clone(&window), data_dir, html, &ipc_handler, options) {
                Ok(built) => return Ok(built),
                Err(err) => {
                    crate::log_warn!(
                        "WebView 初期化に失敗 (attempt={}, dir={}): {:#}",
                        attempt + 1,
                        data_dir.display(),
                        err
                    );
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("WebView 初期化に失敗")))
            .with_context(|| format!("WebView2 の初期化に失敗 ({data_dir_suffix})"))
    }

    /// 指定データディレクトリで `WebView` を1回構築する
    fn build_once(
        window: Arc<Window>,
        data_dir: &Path,
        html: &str,
        ipc_handler: &Arc<dyn Fn(wry::http::Request<String>) + Send + Sync + 'static>,
        options: WebViewWindowOptions,
    ) -> anyhow::Result<Self> {
        if let Some(parent) = data_dir.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "WebView2 データ親ディレクトリの作成に失敗: {}",
                    parent.display()
                )
            })?;
        }
        std::fs::create_dir_all(data_dir).with_context(|| {
            format!(
                "WebView2 データディレクトリの作成に失敗: {}",
                data_dir.display()
            )
        })?;

        let mut context = WebContext::new(Some(PathBuf::from(data_dir)));
        let handler = Arc::clone(ipc_handler);
        let html_for_nav = html.to_string();
        let mut builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_html(html)
            .with_default_context_menus(false)
            .with_ipc_handler(move |req| handler(req))
            .with_navigation_handler(move |url| {
                let allowed = is_allowed_webview_navigation(&url, &html_for_nav);
                if !allowed {
                    crate::log_warn!(
                        "WebView のページ遷移を拒否: {}",
                        preview_denied_webview_url(&url)
                    );
                }
                allowed
            })
            .with_new_window_req_handler(|url, _features| {
                crate::log_warn!(
                    "WebView の新規ウィンドウを拒否: {}",
                    preview_denied_webview_url(&url)
                );
                NewWindowResponse::Deny
            })
            .with_download_started_handler(|url, _path| {
                crate::log_warn!(
                    "WebView のダウンロードを拒否: {}",
                    preview_denied_webview_url(&url)
                );
                false
            });
        if options.transparent {
            builder = builder
                .with_transparent(true)
                .with_background_color((0, 0, 0, 0));
        }
        if let Some(args) = crate::tray::e2e::additional_browser_args() {
            builder = builder.with_additional_browser_args(args);
        }
        let webview = builder
            .build(window.as_ref())
            .with_context(|| format!("WebView の構築に失敗: {}", data_dir.display()))?;

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

    /// ウィンドウを非表示にする (`WebView` は保持する)
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// JavaScript を実行する
    ///
    /// # Arguments
    /// * `script` - 実行する JavaScript
    pub fn evaluate_script(&self, script: &str) {
        webview_script::evaluate_script(&self.webview, "WebView", script);
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
/// 画面の水平中央・上から 1/3 の位置に配置する非表示セレクターウィンドウを生成する
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

    /// 共通 CSS がインライン展開されるようにすること
    #[test]
    fn embed_selector_assets_inlines_common_css() {
        let html = embed_selector_assets(r#"<style>@import url("selector.css");</style>"#);
        assert!(!html.contains(SELECTOR_CSS_IMPORT));
        assert!(html.contains("--bg-color"));
    }

    /// クイックセレクター用バンドル JS がインライン展開されるようにすること
    #[test]
    fn embed_quick_selector_assets_inlines_quick_bundle() {
        let html = embed_quick_selector_assets(
            r#"<style>@import url("selector.css");</style><script src="quick_selector.js"></script>"#,
        );
        assert!(!html.contains(QUICK_SELECTOR_JS_TAG));
        assert!(html.contains("Generated by scripts/common/build-ui.ts"));
        assert!(html.contains("window.refreshModes"));
        assert!(html.contains("FAVORITES_CATEGORY"));
    }

    /// 登録クリップセレクター専用 CSS / バンドル JS がインライン展開されるようにすること
    #[test]
    fn embed_clip_selector_assets_inlines_clip_assets() {
        let html = embed_clip_selector_assets(
            r#"<style>@import url("selector.css");@import url("clip_selector.css");</style><script src="clip_selector.js"></script>"#,
        );
        assert!(!html.contains(CLIP_SELECTOR_CSS_IMPORT));
        assert!(!html.contains(CLIP_SELECTOR_JS_TAG));
        assert!(html.contains("Generated by scripts/common/build-ui.ts"));
        assert!(html.contains("#image-hover-preview"));
        assert!(html.contains("window.refreshItems"));
    }
}

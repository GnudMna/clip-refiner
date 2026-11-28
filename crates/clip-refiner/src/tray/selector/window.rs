//! コマンドパレット風 `WebView` セレクターの共通基盤
//!
//! クイックセレクターと登録クリップセレクターで共有するウィンドウ生成・表示制御を提供する

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder, WindowId};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, COREWEBVIEW2_PROCESS_FAILED_KIND,
    COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, ICoreWebView2_19,
};
use webview2_com::ProcessFailedEventHandler;
use windows::core::Interface;
use wry::{
    NewWindowResponse, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows,
    WebViewExtWindows,
};

use super::webview_policy::{
    apply_webview_csp, inline_script_block, is_allowed_webview_navigation,
    preview_denied_webview_url,
};
use super::webview_script;
use crate::consts;
use crate::tray::dispatch;
use crate::tray::state::{AppEvent, WebViewEvent};

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
// WebView2 データディレクトリ
// ======================================================================
/// 全 `WebView` で共有する `WebView2` データディレクトリ名の識別部分
///
/// 同じディレクトリと同じ環境オプションの `WebView` は browser / GPU プロセスを共有する
const WEBVIEW_DATA_DIR_LABEL: &str = "WebView2";

/// 退避用データディレクトリへ切り替えたか
///
/// 一度切り替えたら、以降の `WebView` も同じディレクトリで作り、プロセスの共有を保つ
static FALLBACK_DIR_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 共有データディレクトリ名を返す (E2E 時は CDP ポートを付与する)
fn webview_data_dir_name() -> String {
    crate::tray::e2e::webview_data_dir_suffix(&format!(
        "{}-{WEBVIEW_DATA_DIR_LABEL}",
        consts::APP_NAME
    ))
}

/// 構築を試すデータディレクトリを試行順に返す
///
/// 退避用へ切り替え済みならそれだけを返す。未切り替えなら直前プロセスのロック解放を待つため
/// 共有ディレクトリを 2 回試し、最後に退避用を試す
fn data_dir_candidates<'a>(
    base_dir: &'a Path,
    fallback_dir: &'a Path,
    fallback_active: bool,
) -> Vec<&'a Path> {
    if fallback_active {
        vec![fallback_dir]
    } else {
        vec![base_dir, base_dir, fallback_dir]
    }
}

// ======================================================================
// プロセスの異常終了
// ======================================================================
/// `WebView` の作り直しが必要なプロセス終了の種類か
///
/// ブラウザ / 描画プロセスの終了では表示を戻せない。GPU やユーティリティのプロセスは
/// `WebView2` が自動で再起動するため、作り直さない
///
/// # Arguments
/// * `kind` - `ProcessFailed` で通知された種類
///
/// # Returns
/// * `bool` - 作り直しが必要なら `true`
fn requires_rebuild(kind: COREWEBVIEW2_PROCESS_FAILED_KIND) -> bool {
    kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED
        || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
}

/// プロセスの異常終了を `WebViewEvent::ProcessFailed` としてイベントループへ送るハンドラを登録する
///
/// ハンドラは `WebView` の破棄とともに解除される
///
/// # Arguments
/// * `webview` - 登録先の `WebView`
/// * `proxy` - イベント送信用プロキシ
/// * `window_id` - 破棄対象を特定するウィンドウ ID
///
/// # Returns
/// * `windows::core::Result<()>` - 登録に失敗した場合は `Err`
fn register_process_failed_handler(
    webview: &WebView,
    proxy: EventLoopProxy<AppEvent>,
    window_id: WindowId,
) -> windows::core::Result<()> {
    let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
        // SAFETY: `args` はイベント中有効な COM 参照で、`kind` は呼び出し中有効な書き込み先
        unsafe { args.ProcessFailedKind(&raw mut kind)? };
        if requires_rebuild(kind) {
            crate::log_warn!(
                "WebView2 のプロセスが異常終了したため WebView を破棄します (kind={})",
                kind.0
            );
            dispatch::send_app_event(&proxy, WebViewEvent::ProcessFailed(window_id).into());
        } else {
            crate::log_info!("WebView2 の補助プロセスが終了しました (kind={})", kind.0);
        }
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: `webview` は生存中の `WebView` で、`token` は呼び出し中有効な書き込み先
    unsafe {
        webview
            .webview()
            .add_ProcessFailed(&handler, &raw mut token)
    }
}

// ======================================================================
// メモリ使用レベル
// ======================================================================
/// 表示状態に応じた `WebView2` のメモリ使用レベルを返す
///
/// # Arguments
/// * `visible` - ウィンドウを表示するか
///
/// # Returns
/// * `COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL` - 表示中は通常、非表示中は低
fn memory_usage_level(visible: bool) -> COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL {
    if visible {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
    } else {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
    }
}

/// 表示状態に合わせて `WebView2` のメモリ使用レベルを切り替える
///
/// 非表示中は低にして、描画プロセスのメモリを OS へ返しやすくする。
/// 対応しない古いランタイム (`ICoreWebView2_19` 未実装) では何もしない
///
/// # Arguments
/// * `webview` - 対象の `WebView`
/// * `visible` - ウィンドウを表示するか
fn apply_memory_usage_level(webview: &WebView, visible: bool) {
    let Ok(webview) = webview.webview().cast::<ICoreWebView2_19>() else {
        return;
    };
    // SAFETY: `webview` は生存中の `WebView` から取得した COM 参照
    if let Err(err) = unsafe { webview.SetMemoryUsageTargetLevel(memory_usage_level(visible)) } {
        crate::log_debug!("WebView のメモリ使用レベルを変更できません: {err}");
    }
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
    /// データディレクトリは全 `WebView` で共有し、browser / GPU プロセスを 1 組にまとめる。
    /// 直前プロセスの `WebView2` ユーザーデータロックで失敗した場合に備え、
    /// 同一ディレクトリ再試行のあとプロセス固有ディレクトリへフォールバックする。
    /// プロセスの異常終了は `WebViewEvent::ProcessFailed` で通知する。
    /// 外部 URL へのページ遷移・`window.open`・ダウンロードは拒否し、
    /// 右クリックの既定コンテキストメニューも無効化する
    ///
    /// # Arguments
    /// * `window` - ホストとなる `tao` ウィンドウ
    /// * `html` - 初期表示 HTML
    /// * `proxy` - プロセスの異常終了を送るイベントプロキシ
    /// * `ipc_handler` - JavaScript からの IPC コールバック
    /// * `options` - 透明化などの表示オプション
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 成功時は `WebSelectorWindow`、失敗時はエラー
    pub fn build(
        window: Window,
        html: &str,
        proxy: &EventLoopProxy<AppEvent>,
        ipc_handler: impl Fn(wry::http::Request<String>) + Send + Sync + 'static,
        options: WebViewWindowOptions,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let data_dir_name = webview_data_dir_name();
        let base_dir = std::env::temp_dir().join(&data_dir_name);
        let fallback_dir = base_dir.join(format!("pid-{}", std::process::id()));
        let ipc_handler: Arc<dyn Fn(wry::http::Request<String>) + Send + Sync + 'static> =
            Arc::new(ipc_handler);
        let candidates = data_dir_candidates(
            &base_dir,
            &fallback_dir,
            FALLBACK_DIR_ACTIVE.load(Ordering::Relaxed),
        );

        let mut last_error = None;
        for (attempt, data_dir) in candidates.into_iter().enumerate() {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(120));
            }
            match Self::build_once(Arc::clone(&window), data_dir, html, &ipc_handler, options) {
                Ok(built) => {
                    if data_dir == fallback_dir {
                        FALLBACK_DIR_ACTIVE.store(true, Ordering::Relaxed);
                    }
                    if let Err(err) =
                        register_process_failed_handler(&built.webview, proxy.clone(), built.id())
                    {
                        crate::log_warn!("WebView のプロセス異常終了の監視を登録できません: {err}");
                    }
                    return Ok(built);
                }
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
            .with_context(|| format!("WebView2 の初期化に失敗 ({data_dir_name})"))
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
    /// 表示の前にメモリ使用レベルを通常へ戻す
    ///
    /// # Arguments
    /// * `script` - 表示直後に実行する JavaScript
    pub fn show_with_script(&self, script: &str) {
        apply_memory_usage_level(&self.webview, true);
        self.window.set_visible(true);
        self.window.set_focus();
        self.evaluate_script(script);
    }

    /// ウィンドウを非表示にする (`WebView` は保持する)
    ///
    /// 非表示のあいだはメモリ使用レベルを低にする
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn hide(&self) {
        self.window.set_visible(false);
        apply_memory_usage_level(&self.webview, false);
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

    /// 退避用へ切り替え前は共有ディレクトリを 2 回試してから退避用を試すこと
    #[test]
    fn data_dir_candidates_retry_shared_dir_before_fallback() {
        let base = Path::new("base");
        let fallback = Path::new("base/pid-1");
        assert_eq!(
            data_dir_candidates(base, fallback, false),
            vec![base, base, fallback]
        );
    }

    /// 退避用へ切り替え後は退避用だけを試し、プロセスの共有を保つこと
    #[test]
    fn data_dir_candidates_keep_fallback_once_active() {
        let base = Path::new("base");
        let fallback = Path::new("base/pid-1");
        assert_eq!(data_dir_candidates(base, fallback, true), vec![fallback]);
    }

    /// 全 `WebView` が同じ共有データディレクトリ名を使うこと
    #[test]
    fn webview_data_dir_name_is_shared() {
        if crate::tray::e2e::cdp_port().is_some() {
            return;
        }
        assert_eq!(
            webview_data_dir_name(),
            format!("{}-WebView2", consts::APP_NAME)
        );
    }

    /// メモリ使用レベルを非表示中だけ低にすること
    #[test]
    fn memory_usage_level_is_low_only_while_hidden() {
        assert_eq!(
            memory_usage_level(true),
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        );
        assert_eq!(
            memory_usage_level(false),
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        );
    }

    /// ブラウザ / 描画プロセスの終了だけを作り直しの対象とすること
    #[test]
    fn requires_rebuild_only_for_browser_and_render_exit() {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
            COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED,
        };

        // 表示を戻せない終了は作り直す
        assert!(requires_rebuild(
            COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED
        ));
        assert!(requires_rebuild(
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
        ));
        // 自動で再起動する補助プロセスや、回復しうる応答なしは作り直さない
        for kind in [
            COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
        ] {
            assert!(!requires_rebuild(kind));
        }
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

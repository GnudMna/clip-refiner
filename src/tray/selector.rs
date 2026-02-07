use std::sync::Arc;

use crate::refiner::RefineMode;
use crate::tray::state::AppEvent;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::WebViewBuilder;

/// クイックセレクター（コマンドパレット）のウィンドウを管理する構造体。
pub struct SelectorWindow {
    webview: wry::WebView,
    window: Arc<Window>,
}

impl SelectorWindow {
    /// 新しい `SelectorWindow` インスタンスを生成する。
    ///
    /// # Arguments
    /// * `window` - セレクターを表示するための Tao ウィンドウ。
    /// * `proxy` - イベントを送信するための EventLoopProxy。
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 生成に成功した場合は `SelectorWindow` インスタンス、失敗した場合はエラー。
    pub fn new(window: Window, proxy: EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let modes_json = RefineMode::to_json_list();

        // HTML content for the Command Palette
        let css = include_str!("../ui/selector.css");
        let html = include_str!("../ui/selector.html")
            .replace("__SELECTOR_CSS__", css)
            .replace("__MODES_JSON__", &modes_json);

        let proxy_clone = proxy.clone();

        let webview = WebViewBuilder::new()
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                let msg = req.body();
                if msg.starts_with("select:") {
                    let mode_str = msg.trim_start_matches("select:");
                    // JSON String to Enum (RefineMode implements Deserialize)
                    if let Ok(mode) =
                        serde_json::from_str::<RefineMode>(&format!("\"{}\"", mode_str))
                    {
                        let _ = proxy_clone.send_event(AppEvent::RequestModeChange(mode));
                    }
                } else if msg == "close" {
                    let _ = proxy_clone.send_event(AppEvent::HideSelector);
                }
            })
            .build(&window)?;

        Ok(Self { webview, window })
    }

    /// セレクターを表示し、現在のモードを反映させる。
    ///
    /// # Arguments
    /// * `current_mode` - 現在アプリケーションで選択されている加工モード。
    pub fn show(&self, current_mode: RefineMode) {
        self.window.set_visible(true);
        self.window.set_focus();
        // UI側の初期化（入力フォーカスと現在のモードの反映）
        let mode_id = serde_json::to_string(&current_mode).unwrap_or_default();
        let script = format!("focusInput({});", mode_id);
        let _ = self.webview.evaluate_script(&script);
    }

    /// セレクターを非表示にする。
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// セレクターが現在表示されているかどうかを確認する。
    ///
    /// # Returns
    /// * `bool` - 表示されている場合は `true`、それ以外は `false`。
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// セレクターウィンドウの ID を取得する。
    ///
    /// # Returns
    /// * `tao::window::WindowId` - ウィンドウ ID。
    pub fn id(&self) -> tao::window::WindowId {
        self.window.id()
    }
}

/// クイックセレクターを初期化して、非表示状態の `SelectorWindow` インスタンスを返す。
///
/// # Arguments
/// * `event_loop` - ウィンドウを作成するためのイベントループのターゲット。
/// * `proxy` - イベントを送信するための EventLoopProxy。
///
/// # Returns
/// * `anyhow::Result<SelectorWindow>` - 初期化に成功した場合は `SelectorWindow` インスタンス、失敗した場合はエラー。
pub fn init_selector(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: EventLoopProxy<AppEvent>,
) -> anyhow::Result<SelectorWindow> {
    use tao::window::WindowBuilder;

    let window = WindowBuilder::new()
        .with_title("ClipRefiner Quick Selector")
        .with_always_on_top(true)
        .with_decorations(false)
        .with_transparent(true)
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(tao::dpi::LogicalSize::new(500.0, 400.0))
        .build(event_loop)?;

    // ウィンドウを画面中央に配置
    if let Some(monitor) = window.current_monitor() {
        let screen_size = monitor.size();
        let window_size = window.outer_size();
        let x = (screen_size.width as i32 - window_size.width as i32) / 2;
        let y = (screen_size.height as i32 - window_size.height as i32) / 3; // やや上寄り
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }

    SelectorWindow::new(window, proxy)
}

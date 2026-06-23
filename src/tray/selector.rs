use std::sync::Arc;

use crate::consts;
use crate::refiner::RefineMode;
use crate::tray::state::AppEvent;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::{WebContext, WebViewBuilder};

// ======================================================================
// セレクタウィンドウ構造体
// ======================================================================
/// クイックセレクタ(モード選択用のコマンドパレット風 UI)を管理する構造体
///
/// `wry` を使用して HTML/JS ベースの UI を透明なウィンドウ上に描画する
pub struct SelectorWindow {
    /// `WebView` インスタンス
    webview: wry::WebView,
    /// 描画先のウィンドウ
    window: Arc<Window>,
    /// `WebView` のコンテキスト
    _context: wry::WebContext,
}

// ======================================================================
// 初期化
// ======================================================================
impl SelectorWindow {
    /// セレクタウィンドウと `WebView` を初期化して生成する
    ///
    /// # Arguments
    /// * `window` - ベースとなる `tao` ウィンドウ
    /// * `proxy` - UIスレッド(イベントループ)へメッセージを送信するためのプロキシ
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 生成に成功した場合は `SelectorWindow` インスタンス、失敗した場合はエラー内容を返す
    pub fn new(window: Window, proxy: &EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let modes_json = RefineMode::to_json_list();

        // コマンドパレット用 HTML を組み立て
        let css = include_str!("../ui/selector.css");
        let html = include_str!("../ui/selector.html")
            .replace("@import url(\"selector.css\");", css)
            .replace(
                r#"<script type="application/json" id="modes-data">[]</script>"#,
                &format!(
                    r#"<script type="application/json" id="modes-data">{modes_json}</script>"#
                ),
            );

        let proxy_clone = proxy.clone();

        let data_dir = std::env::temp_dir().join(format!("{}-WebView2", consts::APP_NAME));

        let mut context = WebContext::new(Some(data_dir));

        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                let msg = req.body();
                if msg.starts_with("select:") {
                    let mode_str = msg.trim_start_matches("select:");
                    // IPC メッセージから RefineMode を復元
                    if let Ok(mode) = serde_json::from_str::<RefineMode>(&format!("\"{mode_str}\""))
                    {
                        let _ = proxy_clone.send_event(AppEvent::RequestModeChange(mode));
                    }
                } else if msg == "close" {
                    let _ = proxy_clone.send_event(AppEvent::HideSelector);
                }
            })
            .build(&window)?;

        Ok(Self {
            webview,
            window,
            _context: context,
        })
    }
}

// ======================================================================
// UI操作
// ======================================================================
impl SelectorWindow {
    /// セレクタを表示し、現在の加工モードを反映させる
    ///
    /// 表示時に UI 側の検索フォームにフォーカスを合わせ、現在のモードをハイライトする
    ///
    /// # Arguments
    /// * `current_mode` - 現在アプリケーションで選択されている加工モード
    pub fn show(&self, current_mode: RefineMode) {
        self.window.set_visible(true);
        self.window.set_focus();
        // UI側の初期化(入力フォーカスと現在のモードの反映)
        let mode_id = serde_json::to_string(&current_mode).unwrap_or_default();
        let script = format!("focusInput({mode_id});");
        let _ = self.webview.evaluate_script(&script);
    }

    /// セレクタを非表示にする
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// セレクタが現在表示されているかどうかを確認する
    ///
    /// # Returns
    /// * `bool` - 表示中であれば `true`、そうでなければ `false`
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// セレクタウィンドウの内部 ID を取得する
    ///
    /// # Returns
    /// * `tao::window::WindowId` - ウィンドウ ID
    pub fn id(&self) -> tao::window::WindowId {
        self.window.id()
    }
}

// ======================================================================
// ウィンドウ初期化
// ======================================================================
/// セレクタウィンドウを初期化して、非表示状態のインスタンスを作成する
///
/// ウィンドウの各種属性(透明度、フレームなしなど)を設定し、画面中央に配置する
///
/// # Arguments
/// * `event_loop` - ウィンドウ作成用のイベントループ
/// * `proxy` - イベント送信用プロキシ
///
/// # Returns
/// * `anyhow::Result<SelectorWindow>` - 初期化に成功した場合は `SelectorWindow` インスタンス、失敗した場合はエラー内容を返す
pub fn init_selector(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: &EventLoopProxy<AppEvent>,
) -> anyhow::Result<SelectorWindow> {
    use tao::window::WindowBuilder;

    let window = WindowBuilder::new()
        .with_title(format!("{} Quick Selector", consts::APP_NAME))
        .with_always_on_top(true)
        .with_decorations(false)
        .with_transparent(true)
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 600.0))
        .build(event_loop)?;

    // ウィンドウを画面中央に配置
    if let Some(monitor) = window.current_monitor() {
        let screen_size = monitor.size();
        let window_size = window.outer_size();
        let x = (screen_size.width.cast_signed() - window_size.width.cast_signed()) / 2;
        let y = (screen_size.height.cast_signed() - window_size.height.cast_signed()) / 3; // やや上寄り
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }

    SelectorWindow::new(window, proxy)
}

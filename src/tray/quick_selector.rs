use std::sync::Arc;

use crate::config::FavoriteMoveDirection;
use crate::consts;
use crate::refiner::RefineMode;
use crate::tray::state::AppEvent;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::{WebContext, WebViewBuilder};

// ======================================================================
// クイックセレクター IPC
// ======================================================================
/// クイックセレクターから送られる IPC メッセージの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickSelectorIpcAction {
    /// 加工モードが選択された
    SelectMode(RefineMode),
    /// お気に入り登録状態を切り替える
    ToggleFavorite(RefineMode),
    /// お気に入りの表示順を変更する
    MoveFavorite(RefineMode, FavoriteMoveDirection),
    /// クイックセレクターを閉じる
    Close,
}

/// IPC メッセージ文字列を解釈する
pub(crate) fn parse_quick_selector_ipc_message(msg: &str) -> Option<QuickSelectorIpcAction> {
    if let Some(mode_str) = msg.strip_prefix("select:") {
        serde_json::from_str::<RefineMode>(&format!("\"{mode_str}\""))
            .ok()
            .map(QuickSelectorIpcAction::SelectMode)
    } else if let Some(mode_str) = msg.strip_prefix("toggle-favorite:") {
        serde_json::from_str::<RefineMode>(&format!("\"{mode_str}\""))
            .ok()
            .map(QuickSelectorIpcAction::ToggleFavorite)
    } else if let Some(rest) = msg.strip_prefix("move-favorite:") {
        let (mode_str, direction_str) = rest.split_once(':')?;
        let mode = serde_json::from_str::<RefineMode>(&format!("\"{mode_str}\"")).ok()?;
        let direction = match direction_str {
            "up" => FavoriteMoveDirection::Up,
            "down" => FavoriteMoveDirection::Down,
            _ => return None,
        };
        Some(QuickSelectorIpcAction::MoveFavorite(mode, direction))
    } else if msg == "close" {
        Some(QuickSelectorIpcAction::Close)
    } else {
        None
    }
}

/// クイックセレクター用 HTML を組み立てる
pub(crate) fn assemble_quick_selector_html(modes_json: &str) -> String {
    let css = include_str!("../ui/selector.css");
    include_str!("../ui/quick_selector.html")
        .replace("@import url(\"selector.css\");", css)
        .replace(
            r#"<script type="application/json" id="modes-data">[]</script>"#,
            &format!(r#"<script type="application/json" id="modes-data">{modes_json}</script>"#),
        )
}

/// クイックセレクター表示時に実行するフォーカス用スクリプトを生成する
pub(crate) fn quick_selector_focus_script(current_mode: RefineMode, modes_json: &str) -> String {
    let mode_id = serde_json::to_string(&current_mode).unwrap_or_default();
    format!("focusInput({mode_id}, {modes_json});")
}

/// モード一覧の再描画用スクリプトを生成する
pub(crate) fn quick_selector_refresh_script(current_mode: RefineMode, modes_json: &str) -> String {
    let mode_id = serde_json::to_string(&current_mode).unwrap_or_default();
    format!("refreshModes({mode_id}, {modes_json});")
}

// ======================================================================
// クイックセレクターウィンドウ構造体
// ======================================================================
/// 加工モード選択用クイックセレクター (コマンドパレット風 UI) を管理する構造体
///
/// `wry` を使用して HTML/JS ベースの UI を透明なウィンドウ上に描画する
pub struct QuickSelectorWindow {
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
impl QuickSelectorWindow {
    /// クイックセレクターウィンドウと `WebView` を初期化して生成する
    ///
    /// # Arguments
    /// * `window` - ベースとなる `tao` ウィンドウ
    /// * `proxy` - UIスレッド(イベントループ)へメッセージを送信するためのプロキシ
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 生成に成功した場合は `QuickSelectorWindow` インスタンス、失敗した場合はエラー内容を返す
    pub fn new(window: Window, proxy: &EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let modes_json = RefineMode::to_json_list(&[]);

        let html = assemble_quick_selector_html(&modes_json);

        let proxy_clone = proxy.clone();

        let data_dir =
            std::env::temp_dir().join(format!("{}-QuickSelector-WebView2", consts::APP_NAME));

        let mut context = WebContext::new(Some(data_dir));

        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                let msg = req.body();
                match parse_quick_selector_ipc_message(msg) {
                    Some(QuickSelectorIpcAction::SelectMode(mode)) => {
                        let _ = proxy_clone.send_event(AppEvent::RequestModeChange(mode));
                    }
                    Some(QuickSelectorIpcAction::ToggleFavorite(mode)) => {
                        let _ = proxy_clone.send_event(AppEvent::RequestFavoriteToggle(mode));
                    }
                    Some(QuickSelectorIpcAction::MoveFavorite(mode, direction)) => {
                        let _ =
                            proxy_clone.send_event(AppEvent::RequestFavoriteMove(mode, direction));
                    }
                    Some(QuickSelectorIpcAction::Close) => {
                        let _ = proxy_clone.send_event(AppEvent::HideQuickSelector);
                    }
                    None => {}
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
impl QuickSelectorWindow {
    /// クイックセレクターを表示し、現在の加工モードを反映させる
    ///
    /// 表示時に UI 側の検索フォームにフォーカスを合わせ、現在のモードをハイライトする
    ///
    /// # Arguments
    /// * `current_mode` - 現在アプリケーションで選択されている加工モード
    /// * `modes_json` - モード一覧 JSON
    pub fn show(&self, current_mode: RefineMode, modes_json: &str) {
        self.window.set_visible(true);
        self.window.set_focus();
        let script = quick_selector_focus_script(current_mode, modes_json);
        let _ = self.webview.evaluate_script(&script);
    }

    /// モード一覧を再描画する
    pub fn refresh_modes(&self, modes_json: &str, current_mode: RefineMode) {
        let script = quick_selector_refresh_script(current_mode, modes_json);
        let _ = self.webview.evaluate_script(&script);
    }

    /// クイックセレクターを非表示にする
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// クイックセレクターが現在表示されているかどうかを確認する
    ///
    /// # Returns
    /// * `bool` - 表示中であれば `true`、そうでなければ `false`
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// クイックセレクターウィンドウの内部 ID を取得する
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
/// クイックセレクターウィンドウを初期化して、非表示状態のインスタンスを作成する
///
/// ウィンドウの各種属性(透明度、フレームなしなど)を設定し、画面中央に配置する
///
/// # Arguments
/// * `event_loop` - ウィンドウ作成用のイベントループ
/// * `proxy` - イベント送信用プロキシ
///
/// # Returns
/// * `anyhow::Result<QuickSelectorWindow>` - 初期化に成功した場合は `QuickSelectorWindow` インスタンス、失敗した場合はエラー内容を返す
pub fn init_quick_selector(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: &EventLoopProxy<AppEvent>,
) -> anyhow::Result<QuickSelectorWindow> {
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

    if let Some(monitor) = window.current_monitor() {
        let screen_size = monitor.size();
        let window_size = window.outer_size();
        let x = (screen_size.width.cast_signed() - window_size.width.cast_signed()) / 2;
        let y = (screen_size.height.cast_signed() - window_size.height.cast_signed()) / 3;
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }

    QuickSelectorWindow::new(window, proxy)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `select:` プレフィックス付きメッセージからモードを復元できること
    #[test]
    fn parse_ipc_select_mode() {
        assert_eq!(
            parse_quick_selector_ipc_message("select:UrlDecode"),
            Some(QuickSelectorIpcAction::SelectMode(RefineMode::UrlDecode))
        );
    }

    /// `toggle-favorite:` プレフィックス付きメッセージを解釈できること
    #[test]
    fn parse_ipc_toggle_favorite() {
        assert_eq!(
            parse_quick_selector_ipc_message("toggle-favorite:Trim"),
            Some(QuickSelectorIpcAction::ToggleFavorite(RefineMode::Trim))
        );
    }

    /// `move-favorite:` プレフィックス付きメッセージを解釈できること
    #[test]
    fn parse_ipc_move_favorite() {
        assert_eq!(
            parse_quick_selector_ipc_message("move-favorite:Trim:up"),
            Some(QuickSelectorIpcAction::MoveFavorite(
                RefineMode::Trim,
                FavoriteMoveDirection::Up
            ))
        );
        assert_eq!(
            parse_quick_selector_ipc_message("move-favorite:UrlDecode:down"),
            Some(QuickSelectorIpcAction::MoveFavorite(
                RefineMode::UrlDecode,
                FavoriteMoveDirection::Down
            ))
        );
    }

    /// `close` メッセージを解釈できること
    #[test]
    fn parse_ipc_close() {
        assert_eq!(
            parse_quick_selector_ipc_message("close"),
            Some(QuickSelectorIpcAction::Close)
        );
    }

    /// 不正なメッセージは `None` を返すこと
    #[test]
    fn parse_ipc_unknown_returns_none() {
        assert_eq!(parse_quick_selector_ipc_message("invalid"), None);
        assert_eq!(parse_quick_selector_ipc_message("select:not-a-mode"), None);
        assert_eq!(
            parse_quick_selector_ipc_message("toggle-favorite:not-a-mode"),
            None
        );
        assert_eq!(
            parse_quick_selector_ipc_message("move-favorite:Trim:left"),
            None
        );
    }

    /// 生成 HTML にモード JSON と CSS が埋め込まれること
    #[test]
    fn assemble_quick_selector_html_embeds_modes_and_css() {
        let html = assemble_quick_selector_html(r#"[{"id":"trim"}]"#);
        assert!(html.contains(r#"[{"id":"trim"}]"#));
        assert!(!html.contains(r#"@import url("selector.css");"#));
        assert!(html.contains("focusInput"));
        assert!(html.contains("クイックセレクター"));
    }

    /// フォーカス用スクリプトに現在モードと一覧 JSON が含まれること
    #[test]
    fn quick_selector_focus_script_contains_mode_json() {
        let script = quick_selector_focus_script(RefineMode::JsonFormat, "[]");
        assert!(script.starts_with("focusInput("));
        assert!(script.contains("JsonFormat"));
        assert!(script.ends_with(");"));
    }
}

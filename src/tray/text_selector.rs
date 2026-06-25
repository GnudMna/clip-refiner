use std::sync::Arc;

use crate::consts;
use crate::tray::state::AppEvent;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::{WebContext, WebViewBuilder};

// ======================================================================
// テキストセレクタ IPC
// ======================================================================
/// テキストセレクタから送られる IPC メッセージの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextSelectorIpcAction {
    /// 登録文字列が選択された
    SelectText(usize),
    /// クリップボードの内容を登録文字列として保存する
    Register,
    /// 登録文字列を削除する
    DeleteText(usize),
    /// セレクタを閉じる
    Close,
}

/// IPC メッセージ文字列を解釈する
pub(crate) fn parse_text_selector_ipc_message(msg: &str) -> Option<TextSelectorIpcAction> {
    if let Some(index_str) = msg.strip_prefix("select:") {
        index_str
            .parse::<usize>()
            .ok()
            .map(TextSelectorIpcAction::SelectText)
    } else if msg == "close" {
        Some(TextSelectorIpcAction::Close)
    } else if msg == "register" {
        Some(TextSelectorIpcAction::Register)
    } else if let Some(index_str) = msg.strip_prefix("delete:") {
        index_str
            .parse::<usize>()
            .ok()
            .map(TextSelectorIpcAction::DeleteText)
    } else {
        None
    }
}

/// テキストセレクタ用 HTML を組み立てる
pub(crate) fn assemble_text_selector_html() -> String {
    let css = include_str!("../ui/selector.css");
    include_str!("../ui/text_selector.html").replace("@import url(\"selector.css\");", css)
}

/// テキストセレクタ表示時に実行するフォーカス用スクリプトを生成する
pub(crate) fn text_selector_focus_script(texts_json: &str) -> String {
    format!("focusInput({texts_json});")
}

/// 登録文字列一覧の再描画用スクリプトを生成する
pub(crate) fn text_selector_refresh_script(texts_json: &str) -> String {
    format!("refreshItems({texts_json});")
}

// ======================================================================
// テキストセレクタウィンドウ構造体
// ======================================================================
/// 登録文字列選択用のコマンドパレット風 UI を管理する構造体
pub struct TextSelectorWindow {
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
impl TextSelectorWindow {
    /// テキストセレクタウィンドウと `WebView` を初期化して生成する
    pub fn new(window: Window, proxy: &EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let window = Arc::new(window);
        let html = assemble_text_selector_html();
        let proxy_clone = proxy.clone();

        let data_dir =
            std::env::temp_dir().join(format!("{}-TextSelector-WebView2", consts::APP_NAME));

        let mut context = WebContext::new(Some(data_dir));

        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(html)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                let msg = req.body();
                match parse_text_selector_ipc_message(msg) {
                    Some(TextSelectorIpcAction::SelectText(index)) => {
                        let _ = proxy_clone.send_event(AppEvent::RequestTextCopy(index));
                    }
                    Some(TextSelectorIpcAction::Register) => {
                        let _ = proxy_clone.send_event(AppEvent::RequestTextRegister);
                    }
                    Some(TextSelectorIpcAction::DeleteText(index)) => {
                        let _ = proxy_clone.send_event(AppEvent::RequestTextDelete(index));
                    }
                    Some(TextSelectorIpcAction::Close) => {
                        let _ = proxy_clone.send_event(AppEvent::HideTextSelector);
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
impl TextSelectorWindow {
    /// テキストセレクタを表示し、登録文字列一覧を反映する
    pub fn show(&self, texts_json: &str) {
        self.window.set_visible(true);
        self.window.set_focus();
        let script = text_selector_focus_script(texts_json);
        let _ = self.webview.evaluate_script(&script);
    }

    /// 登録文字列一覧を再描画する (ウィンドウ表示中の更新用)
    pub fn refresh_items(&self, texts_json: &str) {
        let script = text_selector_refresh_script(texts_json);
        let _ = self.webview.evaluate_script(&script);
    }

    /// テキストセレクタを非表示にする
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// テキストセレクタが現在表示されているかどうかを確認する
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// テキストセレクタウィンドウの内部 ID を取得する
    pub fn id(&self) -> tao::window::WindowId {
        self.window.id()
    }
}

// ======================================================================
// ウィンドウ初期化
// ======================================================================
/// テキストセレクタウィンドウを初期化して、非表示状態のインスタンスを作成する
pub fn init_text_selector(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: &EventLoopProxy<AppEvent>,
) -> anyhow::Result<TextSelectorWindow> {
    use tao::window::WindowBuilder;

    let window = WindowBuilder::new()
        .with_title(format!("{} Text Selector", consts::APP_NAME))
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

    TextSelectorWindow::new(window, proxy)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `select:` プレフィックス付きメッセージからインデックスを復元できること
    #[test]
    fn parse_ipc_select_text() {
        assert_eq!(
            parse_text_selector_ipc_message("select:2"),
            Some(TextSelectorIpcAction::SelectText(2))
        );
    }

    /// `close` メッセージを解釈できること
    #[test]
    fn parse_ipc_close() {
        assert_eq!(
            parse_text_selector_ipc_message("close"),
            Some(TextSelectorIpcAction::Close)
        );
    }

    /// `register` メッセージを解釈できること
    #[test]
    fn parse_ipc_register() {
        assert_eq!(
            parse_text_selector_ipc_message("register"),
            Some(TextSelectorIpcAction::Register)
        );
    }

    /// `delete:` プレフィックス付きメッセージからインデックスを復元できること
    #[test]
    fn parse_ipc_delete_text() {
        assert_eq!(
            parse_text_selector_ipc_message("delete:3"),
            Some(TextSelectorIpcAction::DeleteText(3))
        );
    }

    /// 不正なメッセージは `None` を返すこと
    #[test]
    fn parse_ipc_unknown_returns_none() {
        assert_eq!(parse_text_selector_ipc_message("invalid"), None);
        assert_eq!(parse_text_selector_ipc_message("select:not-a-number"), None);
    }

    /// 生成 HTML に CSS が埋め込まれること
    #[test]
    fn assemble_text_selector_html_embeds_css() {
        let html = assemble_text_selector_html();
        assert!(!html.contains(r#"@import url("selector.css");"#));
        assert!(html.contains("focusInput"));
    }

    /// フォーカス用スクリプトに JSON が含まれること
    #[test]
    fn text_selector_focus_script_contains_json() {
        let script = text_selector_focus_script(r#"[{"id":"0","label":"test"}]"#);
        assert!(script.starts_with("focusInput("));
        assert!(script.contains(r#"[{"id":"0","label":"test"}]"#));
        assert!(script.ends_with(");"));
    }

    /// 再描画用スクリプトが `refreshItems` を呼ぶこと
    #[test]
    fn text_selector_refresh_script_calls_refresh_items() {
        let script = text_selector_refresh_script(r"[]");
        assert_eq!(script, "refreshItems([]);");
    }
}

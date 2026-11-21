use super::dispatch;
use super::selector_window::{WebSelectorWindow, build_hidden_selector_window, embed_selector_css};
use super::state::AppEvent;

use crate::consts;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;

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
    embed_selector_css(include_str!("../ui/text_selector.html"))
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
    inner: WebSelectorWindow,
}

// ======================================================================
// 初期化
// ======================================================================
impl TextSelectorWindow {
    /// テキストセレクタウィンドウと `WebView` を初期化して生成する
    pub fn new(window: Window, proxy: &EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let html = assemble_text_selector_html();
        let proxy_clone = proxy.clone();
        let data_dir = format!("{}-TextSelector-WebView2", consts::APP_NAME);

        let inner = WebSelectorWindow::build(window, &data_dir, html, move |req| {
            let msg = req.body();
            match parse_text_selector_ipc_message(msg) {
                Some(TextSelectorIpcAction::SelectText(index)) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::RequestTextCopy(index));
                }
                Some(TextSelectorIpcAction::Register) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::RequestTextRegister);
                }
                Some(TextSelectorIpcAction::DeleteText(index)) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::RequestTextDelete(index));
                }
                Some(TextSelectorIpcAction::Close) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::HideTextSelector);
                }
                None => {}
            }
        })?;

        Ok(Self { inner })
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl TextSelectorWindow {
    /// テキストセレクタを表示し、登録文字列一覧を反映する
    pub fn show(&self, texts_json: &str) {
        let script = text_selector_focus_script(texts_json);
        self.inner.show_with_script(&script);
    }

    /// 登録文字列一覧を再描画する (ウィンドウ表示中の更新用)
    pub fn refresh_items(&self, texts_json: &str) {
        let script = text_selector_refresh_script(texts_json);
        self.inner.evaluate_script(&script);
    }

    /// テキストセレクタを非表示にする
    pub fn hide(&self) {
        self.inner.hide();
    }

    /// テキストセレクタが現在表示されているかどうかを確認する
    pub fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }

    /// テキストセレクタウィンドウの内部 ID を取得する
    pub fn id(&self) -> tao::window::WindowId {
        self.inner.id()
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
    let window = build_hidden_selector_window(
        event_loop,
        &format!("{} Text Selector", consts::APP_NAME),
        520.0,
        600.0,
    )?;
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

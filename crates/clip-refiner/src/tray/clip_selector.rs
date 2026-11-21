use super::dispatch;
use super::selector_window::{
    WebSelectorWindow, WebViewWindowOptions, build_hidden_selector_window,
    embed_clip_selector_assets,
};
use super::state::AppEvent;

use crate::consts;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;

// ======================================================================
// 登録クリップセレクタ IPC
// ======================================================================
/// 登録クリップセレクタから送られる IPC メッセージの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipSelectorIpcAction {
    /// 登録クリップが選択された
    SelectClip(usize),
    /// クリップボードの内容を登録クリップとして保存する
    Register,
    /// 登録クリップを削除する
    DeleteClip(usize),
    /// セレクタを閉じる
    Close,
}

/// IPC メッセージ文字列を解釈する
pub(crate) fn parse_clip_selector_ipc_message(msg: &str) -> Option<ClipSelectorIpcAction> {
    if let Some(index_str) = msg.strip_prefix("select:") {
        index_str
            .parse::<usize>()
            .ok()
            .map(ClipSelectorIpcAction::SelectClip)
    } else if msg == "close" {
        Some(ClipSelectorIpcAction::Close)
    } else if msg == "register" {
        Some(ClipSelectorIpcAction::Register)
    } else if let Some(index_str) = msg.strip_prefix("delete:") {
        index_str
            .parse::<usize>()
            .ok()
            .map(ClipSelectorIpcAction::DeleteClip)
    } else {
        None
    }
}

/// 登録クリップセレクタ用 HTML を組み立てる
pub(crate) fn assemble_clip_selector_html() -> String {
    embed_clip_selector_assets(include_str!("../ui/clip_selector.html"))
}

/// 登録クリップセレクタ表示時に実行するフォーカス用スクリプトを生成する
pub(crate) fn clip_selector_focus_script(clips_json: &str) -> String {
    format!("focusInput({clips_json});")
}

/// 登録クリップ一覧の再描画用スクリプトを生成する
pub(crate) fn clip_selector_refresh_script(clips_json: &str) -> String {
    format!("refreshItems({clips_json});")
}

// ======================================================================
// 登録クリップセレクタウィンドウ構造体
// ======================================================================
/// 登録クリップ選択用のコマンドパレット風 UI を管理する構造体
pub struct ClipSelectorWindow {
    inner: WebSelectorWindow,
}

// ======================================================================
// 初期化
// ======================================================================
impl ClipSelectorWindow {
    /// 登録クリップセレクタウィンドウと `WebView` を初期化して生成する
    ///
    /// # Arguments
    /// * `window` - ベースとなる `tao` ウィンドウ
    /// * `proxy` - UI スレッド (イベントループ) へメッセージを送信するプロキシ
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - 成功時は `ClipSelectorWindow`、失敗時はエラー
    pub fn new(window: Window, proxy: &EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let html = assemble_clip_selector_html();
        let proxy_clone = proxy.clone();
        let data_dir = format!("{}-ClipSelector-WebView2", consts::APP_NAME);

        let inner = WebSelectorWindow::build(
            window,
            &data_dir,
            html,
            move |req| {
                let msg = req.body();
                match parse_clip_selector_ipc_message(msg) {
                    Some(ClipSelectorIpcAction::SelectClip(index)) => {
                        dispatch::send_app_event(&proxy_clone, AppEvent::RequestClipCopy(index));
                    }
                    Some(ClipSelectorIpcAction::Register) => {
                        dispatch::send_app_event(&proxy_clone, AppEvent::RequestClipRegister);
                    }
                    Some(ClipSelectorIpcAction::DeleteClip(index)) => {
                        dispatch::send_app_event(&proxy_clone, AppEvent::RequestClipDelete(index));
                    }
                    Some(ClipSelectorIpcAction::Close) => {
                        dispatch::send_app_event(&proxy_clone, AppEvent::HideClipSelector);
                    }
                    None => {}
                }
            },
            WebViewWindowOptions::default(),
        )?;

        Ok(Self { inner })
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl ClipSelectorWindow {
    /// 登録クリップセレクタを表示し、登録クリップ一覧を反映する
    ///
    /// # Arguments
    /// * `clips_json` - 登録クリップ一覧 JSON
    pub fn show(&self, clips_json: &str) {
        let script = clip_selector_focus_script(clips_json);
        self.inner.show_with_script(&script);
    }

    /// 登録クリップ一覧を再描画する (ウィンドウ表示中の更新用)
    ///
    /// # Arguments
    /// * `clips_json` - 登録クリップ一覧 JSON
    pub fn refresh_items(&self, clips_json: &str) {
        let script = clip_selector_refresh_script(clips_json);
        self.inner.evaluate_script(&script);
    }

    /// 登録クリップセレクタを非表示にする
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    pub fn hide(&self) {
        self.inner.hide();
    }

    /// 登録クリップセレクタが現在表示されているかどうかを確認する
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }

    /// 登録クリップセレクタウィンドウの内部 ID を取得する
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    ///
    /// # Returns
    /// * `tao::window::WindowId` - ウィンドウ ID
    pub fn id(&self) -> tao::window::WindowId {
        self.inner.id()
    }
}

// ======================================================================
// ウィンドウ初期化
// ======================================================================
/// 登録クリップセレクタウィンドウを初期化して、非表示状態のインスタンスを作成する
///
/// # Arguments
/// * `event_loop` - ウィンドウ作成用のイベントループ
/// * `proxy` - イベント送信用プロキシ
///
/// # Returns
/// * `anyhow::Result<ClipSelectorWindow>` - 成功時は `ClipSelectorWindow`、失敗時はエラー
pub fn init_clip_selector(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: &EventLoopProxy<AppEvent>,
) -> anyhow::Result<ClipSelectorWindow> {
    let window = build_hidden_selector_window(
        event_loop,
        &format!("{} Clip Selector", consts::APP_NAME),
        520.0,
        600.0,
    )?;
    ClipSelectorWindow::new(window, proxy)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `select:` プレフィックス付きメッセージからインデックスを復元できるようにすること
    #[test]
    fn parse_ipc_select_clip() {
        assert_eq!(
            parse_clip_selector_ipc_message("select:2"),
            Some(ClipSelectorIpcAction::SelectClip(2))
        );
    }

    /// `close` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_close() {
        assert_eq!(
            parse_clip_selector_ipc_message("close"),
            Some(ClipSelectorIpcAction::Close)
        );
    }

    /// `register` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_register() {
        assert_eq!(
            parse_clip_selector_ipc_message("register"),
            Some(ClipSelectorIpcAction::Register)
        );
    }

    /// `delete:` プレフィックス付きメッセージからインデックスを復元できるようにすること
    #[test]
    fn parse_ipc_delete_clip() {
        assert_eq!(
            parse_clip_selector_ipc_message("delete:3"),
            Some(ClipSelectorIpcAction::DeleteClip(3))
        );
    }

    /// 不正なメッセージは `None` を返すようにすること
    #[test]
    fn parse_ipc_unknown_returns_none() {
        assert_eq!(parse_clip_selector_ipc_message("invalid"), None);
        assert_eq!(parse_clip_selector_ipc_message("select:not-a-number"), None);
    }

    /// 生成 HTML に CSS が埋め込まれようにすること
    #[test]
    fn assemble_clip_selector_html_embeds_css() {
        let html = assemble_clip_selector_html();
        assert!(!html.contains(r#"@import url("selector.css");"#));
        assert!(!html.contains(r#"@import url("clip_selector.css");"#));
        assert!(!html.contains(r#"<script src="selector-common.js"></script>"#));
        assert!(!html.contains(r#"<script src="clip_selector.js"></script>"#));
        assert!(html.contains("window.SelectorCommon"));
        assert!(html.contains("#image-hover-preview"));
        assert!(html.contains("focusInput"));
    }

    /// フォーカス用スクリプトに JSON が含まれるようにすること
    #[test]
    fn clip_selector_focus_script_contains_json() {
        let script = clip_selector_focus_script(r#"[{"id":"0","label":"test"}]"#);
        assert!(script.starts_with("focusInput("));
        assert!(script.contains(r#"[{"id":"0","label":"test"}]"#));
        assert!(script.ends_with(");"));
    }

    /// 再描画用スクリプトが `refreshItems` を呼ようにすること
    #[test]
    fn clip_selector_refresh_script_calls_refresh_items() {
        let script = clip_selector_refresh_script(r"[]");
        assert_eq!(script, "refreshItems([]);");
    }
}

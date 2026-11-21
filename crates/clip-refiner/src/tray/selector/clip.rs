use std::cell::{Cell, RefCell};
use std::time::Instant;

use super::retention::RetainedSelector;
use super::window::{
    WebSelectorWindow, WebViewWindowOptions, build_hidden_selector_window,
    embed_clip_selector_assets,
};
use crate::config::FavoriteMoveDirection;
use crate::consts;
use crate::tray::dispatch;
use crate::tray::state::{AppEvent, ClipsEvent, SelectorEvent};

use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowId};

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
    /// 登録クリップの表示順を変更する
    MoveClip(usize, FavoriteMoveDirection),
    /// 登録クリップのテンプレート展開を切り替える
    SetTemplate(usize, bool),
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
    } else if let Some(rest) = msg.strip_prefix("move:") {
        let (index_str, direction_str) = rest.split_once(':')?;
        let index = index_str.parse::<usize>().ok()?;
        let direction = match direction_str {
            "up" => FavoriteMoveDirection::Up,
            "down" => FavoriteMoveDirection::Down,
            _ => return None,
        };
        Some(ClipSelectorIpcAction::MoveClip(index, direction))
    } else if let Some(rest) = msg.strip_prefix("template:") {
        let (index_str, state_str) = rest.split_once(':')?;
        let index = index_str.parse::<usize>().ok()?;
        let enabled = match state_str {
            "on" => true,
            "off" => false,
            _ => return None,
        };
        Some(ClipSelectorIpcAction::SetTemplate(index, enabled))
    } else {
        None
    }
}

/// 登録クリップセレクタ用 HTML を組み立てる
pub(crate) fn assemble_clip_selector_html() -> String {
    embed_clip_selector_assets(include_str!("../../ui/clip_selector.html"))
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
///
/// `WebView` は初回表示時に生成する。非表示後の保持 / 破棄は
/// `selector_webview_retention` 設定に従う
pub struct ClipSelectorWindow {
    /// イベント送信用プロキシ
    proxy: EventLoopProxy<AppEvent>,
    /// 遅延生成された `WebView` (未生成または破棄後は `None`)
    inner: RefCell<Option<WebSelectorWindow>>,
    /// `DisposeAfterIdle` 用の非表示開始時刻
    idle_hidden_at: Cell<Option<Instant>>,
    /// テスト用の擬似表示フラグ (`WebView` なしで相互排他を検証する)
    #[cfg(test)]
    force_visible: Cell<bool>,
}

// ======================================================================
// 初期化
// ======================================================================
impl ClipSelectorWindow {
    /// `WebView` 未生成の空シェルを生成する
    ///
    /// # Arguments
    /// * `proxy` - UI スレッドへメッセージを送信するプロキシ
    ///
    /// # Returns
    /// * `Self` - 遅延生成用の空シェル
    pub fn new_shell(proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            proxy,
            inner: RefCell::new(None),
            idle_hidden_at: Cell::new(None),
            #[cfg(test)]
            force_visible: Cell::new(false),
        }
    }

    /// `WebView` を構築する
    fn build_webview(
        window: Window,
        proxy: &EventLoopProxy<AppEvent>,
    ) -> anyhow::Result<WebSelectorWindow> {
        let html = assemble_clip_selector_html();
        let proxy_clone = proxy.clone();
        let data_dir = format!("{}-ClipSelector-WebView2", consts::APP_NAME);

        WebSelectorWindow::build(
            window,
            &data_dir,
            &html,
            move |req| {
                let msg = req.body();
                match parse_clip_selector_ipc_message(msg) {
                    Some(ClipSelectorIpcAction::SelectClip(index)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            ClipsEvent::RequestCopy(index).into(),
                        );
                    }
                    Some(ClipSelectorIpcAction::Register) => {
                        dispatch::send_app_event(&proxy_clone, ClipsEvent::RequestRegister.into());
                    }
                    Some(ClipSelectorIpcAction::DeleteClip(index)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            ClipsEvent::RequestDelete(index).into(),
                        );
                    }
                    Some(ClipSelectorIpcAction::MoveClip(index, direction)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            ClipsEvent::RequestMove(index, direction).into(),
                        );
                    }
                    Some(ClipSelectorIpcAction::SetTemplate(index, enabled)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            ClipsEvent::RequestSetTemplate(index, enabled).into(),
                        );
                    }
                    Some(ClipSelectorIpcAction::Close) => {
                        dispatch::send_app_event(&proxy_clone, SelectorEvent::HideClip.into());
                    }
                    None => {}
                }
            },
            WebViewWindowOptions::default(),
        )
    }

    /// 未生成なら `WebView` を生成する
    fn ensure_created(&self, event_loop: &EventLoopWindowTarget<AppEvent>) -> anyhow::Result<()> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }
        let window = build_hidden_selector_window(
            event_loop,
            &format!("{} Clip Selector", consts::APP_NAME),
            520.0,
            600.0,
        )?;
        let webview = Self::build_webview(window, &self.proxy)?;
        *self.inner.borrow_mut() = Some(webview);
        Ok(())
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl ClipSelectorWindow {
    /// 登録クリップセレクタを表示し、登録クリップ一覧を反映する
    ///
    /// 未生成なら `WebView` を遅延生成する
    ///
    /// # Arguments
    /// * `event_loop` - 遅延生成用のイベントループ
    /// * `clips_json` - 登録クリップ一覧 JSON
    pub fn show(&self, event_loop: &EventLoopWindowTarget<AppEvent>, clips_json: &str) {
        self.idle_hidden_at.set(None);
        if let Err(err) = self.ensure_created(event_loop) {
            crate::user_error::notify_webview_window_init_failure("登録クリップ", &err);
            return;
        }
        let script = clip_selector_focus_script(clips_json);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.show_with_script(&script);
        }
    }

    /// 登録クリップ一覧を再描画する (ウィンドウ表示中の更新用)
    ///
    /// `WebView` 未生成時は何もしない
    ///
    /// # Arguments
    /// * `clips_json` - 登録クリップ一覧 JSON
    pub fn refresh_items(&self, clips_json: &str) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let script = clip_selector_refresh_script(clips_json);
        inner.evaluate_script(&script);
    }

    /// 登録クリップセレクタを非表示にする (`WebView` の破棄は呼び出し側の保持方針に任せる)
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    pub fn hide(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.hide();
        }
    }

    /// `WebView` を破棄する (未生成なら何もしない)
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    pub fn dispose(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        self.idle_hidden_at.set(None);
        *self.inner.borrow_mut() = None;
    }

    /// `WebView` が生成済みかどうか
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    ///
    /// # Returns
    /// * `bool` - 生成済みなら `true`
    pub fn is_created(&self) -> bool {
        self.inner.borrow().is_some()
    }

    /// 登録クリップセレクタが現在表示されているかどうかを確認する
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        #[cfg(test)]
        if self.force_visible.get() {
            return true;
        }
        self.inner
            .borrow()
            .as_ref()
            .is_some_and(WebSelectorWindow::is_visible)
    }

    /// テスト用に表示中として扱う (実 `WebView` は作らない)
    ///
    /// # Arguments
    /// * `visible` - 擬似表示するか
    #[cfg(test)]
    pub(crate) fn force_visible_for_test(&self, visible: bool) {
        self.force_visible.set(visible);
    }

    /// 登録クリップセレクタウィンドウの内部 ID を取得する
    ///
    /// # Arguments
    /// * `self` - この `ClipSelectorWindow`
    ///
    /// # Returns
    /// * `Option<WindowId>` - 生成済みならウィンドウ ID、未生成なら `None`
    pub fn id(&self) -> Option<WindowId> {
        self.inner.borrow().as_ref().map(WebSelectorWindow::id)
    }
}

impl RetainedSelector for ClipSelectorWindow {
    fn hide_visual(&self) {
        ClipSelectorWindow::hide(self);
    }

    fn dispose_webview(&self) {
        ClipSelectorWindow::dispose(self);
    }

    fn is_visible(&self) -> bool {
        ClipSelectorWindow::is_visible(self)
    }

    fn is_created(&self) -> bool {
        ClipSelectorWindow::is_created(self)
    }

    fn idle_hidden_at(&self) -> Option<Instant> {
        self.idle_hidden_at.get()
    }

    fn set_idle_hidden_at(&self, at: Option<Instant>) {
        self.idle_hidden_at.set(at);
    }
}

// ======================================================================
// ウィンドウ初期化
// ======================================================================
/// 登録クリップセレクタの空シェルを生成する (`WebView` は初回表示まで作らない)
///
/// # Arguments
/// * `proxy` - イベント送信用プロキシ
///
/// # Returns
/// * `ClipSelectorWindow` - 遅延生成用シェル
pub fn init_clip_selector(proxy: &EventLoopProxy<AppEvent>) -> ClipSelectorWindow {
    ClipSelectorWindow::new_shell(proxy.clone())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::state::test_app_state;

    /// 空シェルは非表示でウィンドウ ID もなく、`hide` / `refresh` / `dispose` が no-op となるようにすること
    #[test]
    fn empty_shell_lifecycle_is_safe() {
        let state = test_app_state();
        let clip = ClipSelectorWindow::new_shell(state.proxy.clone());

        assert!(!clip.is_visible());
        assert!(!clip.is_created());
        assert!(clip.id().is_none());
        clip.refresh_items("[]");
        clip.hide();
        clip.dispose();
        assert!(!clip.is_visible());
        assert!(!clip.is_created());
        assert!(clip.id().is_none());
    }

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

    /// `move:` メッセージからインデックスと移動方向を復元できるようにすること
    #[test]
    fn parse_ipc_move_clip() {
        assert_eq!(
            parse_clip_selector_ipc_message("move:2:up"),
            Some(ClipSelectorIpcAction::MoveClip(
                2,
                FavoriteMoveDirection::Up
            ))
        );
        assert_eq!(
            parse_clip_selector_ipc_message("move:0:down"),
            Some(ClipSelectorIpcAction::MoveClip(
                0,
                FavoriteMoveDirection::Down
            ))
        );
        assert_eq!(parse_clip_selector_ipc_message("move:0:left"), None);
        assert_eq!(parse_clip_selector_ipc_message("move:x:up"), None);
        assert_eq!(parse_clip_selector_ipc_message("move:1"), None);
    }

    /// `template:` メッセージからインデックスと有効・無効を復元できるようにすること
    #[test]
    fn parse_ipc_set_template() {
        assert_eq!(
            parse_clip_selector_ipc_message("template:1:on"),
            Some(ClipSelectorIpcAction::SetTemplate(1, true))
        );
        assert_eq!(
            parse_clip_selector_ipc_message("template:0:off"),
            Some(ClipSelectorIpcAction::SetTemplate(0, false))
        );
        assert_eq!(parse_clip_selector_ipc_message("template:0:yes"), None);
        assert_eq!(parse_clip_selector_ipc_message("template:x:on"), None);
        assert_eq!(parse_clip_selector_ipc_message("template:1"), None);
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
        assert!(!html.contains(r#"<script src="clip_selector.js"></script>"#));
        assert!(html.contains("Generated by scripts/common/build-ui.mjs"));
        assert!(html.contains("#image-hover-preview"));
        assert!(html.contains("focusInput"));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(!html.contains("__WEBVIEW_CSP__"));
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

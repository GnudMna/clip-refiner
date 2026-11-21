use std::cell::{Cell, RefCell};
use std::time::Instant;

use super::retention::RetainedSelector;
use super::window::{
    WebSelectorWindow, WebViewWindowOptions, build_hidden_selector_window,
    embed_quick_selector_assets,
};
use crate::config::FavoriteMoveDirection;
use crate::consts;
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::state::{AppEvent, FavoritesEvent, SelectorEvent};

use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowId};

// ======================================================================
// クイックセレクター IPC
// ======================================================================
/// クイックセレクターから送られる IPC メッセージの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickSelectorIpcAction {
    /// 変換モードが選択された
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
    embed_quick_selector_assets(include_str!("../../ui/quick_selector.html")).replace(
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
/// 変換モード選択用クイックセレクター (コマンドパレット風 UI) を管理する構造体
///
/// `WebView` は初回表示時に生成する。非表示後の保持 / 破棄は
/// `selector_webview_retention` 設定に従う
pub struct QuickSelectorWindow {
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
impl QuickSelectorWindow {
    /// `WebView` 未生成の空シェルを生成する
    ///
    /// # Arguments
    /// * `proxy` - UI スレッド (イベントループ) へメッセージを送信するプロキシ
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
    ///
    /// # Arguments
    /// * `window` - ホストとなる `tao` ウィンドウ
    /// * `proxy` - UI スレッドへメッセージを送信するプロキシ
    ///
    /// # Returns
    /// * `anyhow::Result<WebSelectorWindow>` - 成功時は構築済み `WebView`
    fn build_webview(
        window: Window,
        proxy: &EventLoopProxy<AppEvent>,
    ) -> anyhow::Result<WebSelectorWindow> {
        let modes_json = RefineMode::to_json_list(&[]);
        let html = assemble_quick_selector_html(&modes_json);
        let proxy_clone = proxy.clone();
        let data_dir = format!("{}-QuickSelector-WebView2", consts::APP_NAME);

        WebSelectorWindow::build(
            window,
            &data_dir,
            &html,
            move |req| {
                let msg = req.body();
                match parse_quick_selector_ipc_message(msg) {
                    Some(QuickSelectorIpcAction::SelectMode(mode)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            SelectorEvent::RequestModeChange(mode).into(),
                        );
                    }
                    Some(QuickSelectorIpcAction::ToggleFavorite(mode)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            FavoritesEvent::RequestToggle(mode).into(),
                        );
                    }
                    Some(QuickSelectorIpcAction::MoveFavorite(mode, direction)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            FavoritesEvent::RequestMove(mode, direction).into(),
                        );
                    }
                    Some(QuickSelectorIpcAction::Close) => {
                        dispatch::send_app_event(&proxy_clone, SelectorEvent::HideQuick.into());
                    }
                    None => {}
                }
            },
            WebViewWindowOptions::default(),
        )
    }

    /// 未生成なら `WebView` を生成する
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウ作成用のイベントループ
    ///
    /// # Returns
    /// * `anyhow::Result<()>` - 生成または既存利用に成功した場合は `Ok(())`
    fn ensure_created(&self, event_loop: &EventLoopWindowTarget<AppEvent>) -> anyhow::Result<()> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }
        let window = build_hidden_selector_window(
            event_loop,
            &format!("{} Quick Selector", consts::APP_NAME),
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
impl QuickSelectorWindow {
    /// クイックセレクターを表示し、現在の変換モードを反映させる
    ///
    /// 未生成なら `WebView` を遅延生成する。表示時に検索フォームへフォーカスし、現在モードをハイライトする
    ///
    /// # Arguments
    /// * `event_loop` - 遅延生成用のイベントループ
    /// * `current_mode` - 現在アプリケーションで選択されている変換モード
    /// * `modes_json` - モード一覧 JSON
    pub fn show(
        &self,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        current_mode: RefineMode,
        modes_json: &str,
    ) {
        self.idle_hidden_at.set(None);
        if let Err(err) = self.ensure_created(event_loop) {
            crate::user_error::notify_webview_window_init_failure("クイックセレクター", &err);
            return;
        }
        let script = quick_selector_focus_script(current_mode, modes_json);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.show_with_script(&script);
        }
    }

    /// モード一覧を再描画する
    ///
    /// `WebView` 未生成時は何もしない
    ///
    /// # Arguments
    /// * `modes_json` - モード一覧 JSON
    /// * `current_mode` - 現在選択されている変換モード
    pub fn refresh_modes(&self, modes_json: &str, current_mode: RefineMode) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let script = quick_selector_refresh_script(current_mode, modes_json);
        inner.evaluate_script(&script);
    }

    /// クイックセレクターを非表示にする (`WebView` の破棄は呼び出し側の保持方針に任せる)
    ///
    /// # Arguments
    /// * `self` - この `QuickSelectorWindow`
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
    /// * `self` - この `QuickSelectorWindow`
    pub fn dispose(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        self.idle_hidden_at.set(None);
        *self.inner.borrow_mut() = None;
    }

    /// `WebView` が生成済みかどうか
    ///
    /// # Arguments
    /// * `self` - この `QuickSelectorWindow`
    ///
    /// # Returns
    /// * `bool` - 生成済みなら `true`
    pub fn is_created(&self) -> bool {
        self.inner.borrow().is_some()
    }

    /// クイックセレクターが現在表示されているかどうかを確認する
    ///
    /// # Arguments
    /// * `self` - この `QuickSelectorWindow`
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

    /// クイックセレクターウィンドウの内部 ID を取得する
    ///
    /// # Arguments
    /// * `self` - この `QuickSelectorWindow`
    ///
    /// # Returns
    /// * `Option<WindowId>` - 生成済みならウィンドウ ID、未生成なら `None`
    pub fn id(&self) -> Option<WindowId> {
        self.inner.borrow().as_ref().map(WebSelectorWindow::id)
    }
}

impl RetainedSelector for QuickSelectorWindow {
    fn hide_visual(&self) {
        QuickSelectorWindow::hide(self);
    }

    fn dispose_webview(&self) {
        QuickSelectorWindow::dispose(self);
    }

    fn is_visible(&self) -> bool {
        QuickSelectorWindow::is_visible(self)
    }

    fn is_created(&self) -> bool {
        QuickSelectorWindow::is_created(self)
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
/// クイックセレクターの空シェルを生成する (`WebView` は初回表示まで作らない)
///
/// # Arguments
/// * `proxy` - イベント送信用プロキシ
///
/// # Returns
/// * `QuickSelectorWindow` - 遅延生成用シェル
pub fn init_quick_selector(proxy: &EventLoopProxy<AppEvent>) -> QuickSelectorWindow {
    QuickSelectorWindow::new_shell(proxy.clone())
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
        let quick = QuickSelectorWindow::new_shell(state.proxy.clone());

        assert!(!quick.is_visible());
        assert!(!quick.is_created());
        assert!(quick.id().is_none());
        quick.refresh_modes("[]", RefineMode::Trim);
        quick.hide();
        quick.dispose();
        assert!(!quick.is_visible());
        assert!(!quick.is_created());
        assert!(quick.id().is_none());
    }

    /// `select:` プレフィックス付きメッセージからモードを復元できるようにすること
    #[test]
    fn parse_ipc_select_mode() {
        assert_eq!(
            parse_quick_selector_ipc_message("select:UrlDecode"),
            Some(QuickSelectorIpcAction::SelectMode(RefineMode::UrlDecode))
        );
    }

    /// `toggle-favorite:` プレフィックス付きメッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_toggle_favorite() {
        assert_eq!(
            parse_quick_selector_ipc_message("toggle-favorite:Trim"),
            Some(QuickSelectorIpcAction::ToggleFavorite(RefineMode::Trim))
        );
    }

    /// `move-favorite:` プレフィックス付きメッセージを解釈できるようにすること
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

    /// `close` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_close() {
        assert_eq!(
            parse_quick_selector_ipc_message("close"),
            Some(QuickSelectorIpcAction::Close)
        );
    }

    /// 不正なメッセージは `None` を返すようにすること
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

    /// 生成 HTML にモード JSON と CSS が埋め込まれようにすること
    #[test]
    fn assemble_quick_selector_html_embeds_modes_and_css() {
        let html = assemble_quick_selector_html(r#"[{"id":"trim"}]"#);
        assert!(html.contains(r#"[{"id":"trim"}]"#));
        assert!(!html.contains(r#"@import url("selector.css");"#));
        assert!(!html.contains(r#"<script src="selector-common.js"></script>"#));
        assert!(!html.contains(r#"<script src="quick_selector_logic.js"></script>"#));
        assert!(!html.contains(r#"<script src="quick_selector.js"></script>"#));
        assert!(html.contains("window.QuickSelectorLogic"));
        assert!(html.contains("window.SelectorCommon"));
        assert!(html.contains("focusInput"));
        assert!(html.contains("クイックセレクター"));
    }

    /// フォーカス用スクリプトに現在モードと一覧 JSON が含まれるようにすること
    #[test]
    fn quick_selector_focus_script_contains_mode_json() {
        let script = quick_selector_focus_script(RefineMode::JsonFormat, "[]");
        assert!(script.starts_with("focusInput("));
        assert!(script.contains("JsonFormat"));
        assert!(script.ends_with(");"));
    }
}

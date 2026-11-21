use super::dispatch;
use super::selector_window::{
    WebSelectorWindow, build_hidden_selector_window, embed_selector_assets,
};
use super::state::AppEvent;

use crate::config::FavoriteMoveDirection;
use crate::consts;
use crate::refiner::RefineMode;

use tao::event_loop::EventLoopProxy;
use tao::window::Window;

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
    embed_selector_assets(include_str!("../ui/quick_selector.html")).replace(
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
    inner: WebSelectorWindow,
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
        let modes_json = RefineMode::to_json_list(&[]);
        let html = assemble_quick_selector_html(&modes_json);
        let proxy_clone = proxy.clone();
        let data_dir = format!("{}-QuickSelector-WebView2", consts::APP_NAME);

        let inner = WebSelectorWindow::build(window, &data_dir, html, move |req| {
            let msg = req.body();
            match parse_quick_selector_ipc_message(msg) {
                Some(QuickSelectorIpcAction::SelectMode(mode)) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::RequestModeChange(mode));
                }
                Some(QuickSelectorIpcAction::ToggleFavorite(mode)) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::RequestFavoriteToggle(mode));
                }
                Some(QuickSelectorIpcAction::MoveFavorite(mode, direction)) => {
                    dispatch::send_app_event(
                        &proxy_clone,
                        AppEvent::RequestFavoriteMove(mode, direction),
                    );
                }
                Some(QuickSelectorIpcAction::Close) => {
                    dispatch::send_app_event(&proxy_clone, AppEvent::HideQuickSelector);
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
impl QuickSelectorWindow {
    /// クイックセレクターを表示し、現在の加工モードを反映させる
    ///
    /// 表示時に UI 側の検索フォームにフォーカスを合わせ、現在のモードをハイライトする
    ///
    /// # Arguments
    /// * `current_mode` - 現在アプリケーションで選択されている加工モード
    /// * `modes_json` - モード一覧 JSON
    pub fn show(&self, current_mode: RefineMode, modes_json: &str) {
        let script = quick_selector_focus_script(current_mode, modes_json);
        self.inner.show_with_script(&script);
    }

    /// モード一覧を再描画する
    pub fn refresh_modes(&self, modes_json: &str, current_mode: RefineMode) {
        let script = quick_selector_refresh_script(current_mode, modes_json);
        self.inner.evaluate_script(&script);
    }

    /// クイックセレクターを非表示にする
    pub fn hide(&self) {
        self.inner.hide();
    }

    /// クイックセレクターが現在表示されているかどうかを確認する
    pub fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }

    /// クイックセレクターウィンドウの内部 ID を取得する
    pub fn id(&self) -> tao::window::WindowId {
        self.inner.id()
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
    let window = build_hidden_selector_window(
        event_loop,
        &format!("{} Quick Selector", consts::APP_NAME),
        520.0,
        600.0,
    )?;
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
        assert!(!html.contains(r#"<script src="selector-common.js"></script>"#));
        assert!(html.contains("window.SelectorCommon"));
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

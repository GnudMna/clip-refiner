use std::sync::{Arc, Mutex};

use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowId};

use super::lazy::{HasLazySelector, LazySelector};
use super::window::{WebSelectorWindow, WebViewWindowOptions, embed_quick_selector_assets};
use crate::config::FavoriteMoveDirection;
use crate::consts;
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::state::{AppEvent, FavoritesEvent, LockExt, SelectorEvent};

// ======================================================================
// クイックセレクター IPC
// ======================================================================
/// クイックセレクターから送られる IPC メッセージの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickSelectorIpcAction {
    /// 自動変換モードとして選択された
    SelectMode(RefineMode),
    /// 現在のクリップボードへ一回だけ適用する
    ApplyMode(RefineMode),
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
    } else if let Some(mode_str) = msg.strip_prefix("apply:") {
        serde_json::from_str::<RefineMode>(&format!("\"{mode_str}\""))
            .ok()
            .map(QuickSelectorIpcAction::ApplyMode)
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

/// 展開中カテゴリとして受け付ける最大件数 (カテゴリ数より十分大きい)
const MAX_EXPANDED_CATEGORIES: usize = 64;
/// 展開中カテゴリ名として受け付ける最大文字数
const MAX_EXPANDED_CATEGORY_CHARS: usize = 64;

/// `expanded-categories:` メッセージから展開中のカテゴリ名を取り出す
///
/// 上限を超える件数・文字数のカテゴリ名は捨てる
///
/// # Arguments
/// * `msg` - IPC メッセージ文字列
///
/// # Returns
/// * `Option<Vec<String>>` - 展開中カテゴリ名。対象外または不正な JSON の場合は `None`
pub(crate) fn parse_expanded_categories_message(msg: &str) -> Option<Vec<String>> {
    let json = msg.strip_prefix("expanded-categories:")?;
    let categories = serde_json::from_str::<Vec<String>>(json).ok()?;
    Some(
        categories
            .into_iter()
            .filter(|category| category.chars().count() <= MAX_EXPANDED_CATEGORY_CHARS)
            .take(MAX_EXPANDED_CATEGORIES)
            .collect(),
    )
}

/// クイックセレクター用 HTML を組み立てる
pub(crate) fn assemble_quick_selector_html(modes_json: &str) -> String {
    embed_quick_selector_assets(include_str!("../../ui/selector/quick_selector.html")).replace(
        r#"<script type="application/json" id="modes-data">[]</script>"#,
        &format!(r#"<script type="application/json" id="modes-data">{modes_json}</script>"#),
    )
}

/// クイックセレクター表示時に実行するフォーカス用スクリプトを生成する
pub(crate) fn quick_selector_focus_script(
    current_mode: Option<RefineMode>,
    modes_json: &str,
    expanded_categories: &[String],
) -> String {
    let mode_id = serde_json::to_string(&current_mode).unwrap_or_default();
    let expanded = serde_json::to_string(expanded_categories).unwrap_or_else(|_| "[]".into());
    format!("focusInput({mode_id}, {modes_json}, {expanded});")
}

/// モード一覧の再描画用スクリプトを生成する
pub(crate) fn quick_selector_refresh_script(
    current_mode: Option<RefineMode>,
    modes_json: &str,
) -> String {
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
    /// 遅延生成される `WebView` とその保持状態
    lazy: LazySelector,
    /// 展開中のカテゴリ名 (`WebView` を破棄しても起動中は保持する)
    expanded_categories: Arc<Mutex<Vec<String>>>,
}

impl HasLazySelector for QuickSelectorWindow {
    fn lazy(&self) -> &LazySelector {
        &self.lazy
    }
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
            lazy: LazySelector::new(),
            expanded_categories: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// `WebView` を構築する
    ///
    /// # Arguments
    /// * `window` - ホストとなる `tao` ウィンドウ
    /// * `proxy` - UI スレッドへメッセージを送信するプロキシ
    /// * `expanded_categories` - `WebView` から通知された展開中カテゴリの保存先
    ///
    /// # Returns
    /// * `anyhow::Result<WebSelectorWindow>` - 成功時は構築済み `WebView`
    fn build_webview(
        window: Window,
        proxy: &EventLoopProxy<AppEvent>,
        expanded_categories: Arc<Mutex<Vec<String>>>,
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
                if let Some(categories) = parse_expanded_categories_message(msg) {
                    *expanded_categories.lock_ignore_poison() = categories;
                    return;
                }
                match parse_quick_selector_ipc_message(msg) {
                    Some(QuickSelectorIpcAction::SelectMode(mode)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            SelectorEvent::RequestModeChange(mode).into(),
                        );
                    }
                    Some(QuickSelectorIpcAction::ApplyMode(mode)) => {
                        dispatch::send_app_event(
                            &proxy_clone,
                            SelectorEvent::RequestApplyOnce(mode).into(),
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
    /// * `current_mode` - 自動変換で使用中の変換モード (自動変換が無効なら `None`)
    /// * `modes_json` - モード一覧 JSON
    pub fn show(
        &self,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        current_mode: Option<RefineMode>,
        modes_json: &str,
    ) {
        let title = format!("{} Quick Selector", consts::APP_NAME);
        let script = quick_selector_focus_script(
            current_mode,
            modes_json,
            &self.expanded_categories.lock_ignore_poison(),
        );
        if let Err(err) = self.lazy.show(
            event_loop,
            &title,
            |window| {
                Self::build_webview(window, &self.proxy, Arc::clone(&self.expanded_categories))
            },
            &script,
        ) {
            crate::user_error::notify_webview_window_init_failure("クイックセレクター", &err);
        }
    }

    /// モード一覧を再描画する
    ///
    /// `WebView` 未生成時は何もしない
    ///
    /// # Arguments
    /// * `modes_json` - モード一覧 JSON
    /// * `current_mode` - 自動変換で使用中の変換モード (自動変換が無効なら `None`)
    pub fn refresh_modes(&self, modes_json: &str, current_mode: Option<RefineMode>) {
        self.lazy
            .evaluate_script(&quick_selector_refresh_script(current_mode, modes_json));
    }

    /// クイックセレクターが現在表示されているかどうかを確認する
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        self.lazy.is_visible()
    }

    /// クイックセレクターウィンドウの内部 ID を取得する
    ///
    /// # Returns
    /// * `Option<WindowId>` - 生成済みならウィンドウ ID、未生成なら `None`
    pub fn id(&self) -> Option<WindowId> {
        self.lazy.id()
    }

    /// テスト用に表示中として扱う (実 `WebView` は作らない)
    ///
    /// # Arguments
    /// * `visible` - 擬似表示するか
    #[cfg(test)]
    pub(crate) fn force_visible_for_test(&self, visible: bool) {
        self.lazy.force_visible_for_test(visible);
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
        assert!(!quick.lazy().is_created());
        assert!(quick.id().is_none());
        quick.refresh_modes("[]", Some(RefineMode::Trim));
        quick.lazy().hide();
        quick.lazy().dispose();
        assert!(!quick.is_visible());
        assert!(!quick.lazy().is_created());
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

    /// `apply:` プレフィックス付きメッセージを一回適用として解釈できるようにすること
    #[test]
    fn parse_ipc_apply_mode() {
        assert_eq!(
            parse_quick_selector_ipc_message("apply:JsonFormat"),
            Some(QuickSelectorIpcAction::ApplyMode(RefineMode::JsonFormat))
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
        assert_eq!(parse_quick_selector_ipc_message("apply:not-a-mode"), None);
        assert_eq!(
            parse_quick_selector_ipc_message("toggle-favorite:not-a-mode"),
            None
        );
        assert_eq!(
            parse_quick_selector_ipc_message("move-favorite:Trim:left"),
            None
        );
    }

    /// 生成 HTML にモード JSON・CSS・JS・CSP が埋め込まれるようにすること
    #[test]
    fn assemble_quick_selector_html_embeds_modes_and_css() {
        let html = assemble_quick_selector_html(r#"[{"id":"trim"}]"#);
        assert!(html.contains(r#"[{"id":"trim"}]"#));
        assert!(!html.contains(r#"@import url("selector.css");"#));
        assert!(!html.contains(r#"<script src="quick_selector.js"></script>"#));
        assert!(html.contains("Generated by scripts/common/build-ui.ts"));
        assert!(html.contains("focusInput"));
        assert!(html.contains("クイックセレクター"));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(!html.contains("__WEBVIEW_CSP__"));
        crate::tray::selector::assert_inline_scripts_allowed_by_csp(&html);
    }

    /// フォーカス用スクリプトに現在モードと一覧 JSON が含まれるようにすること
    #[test]
    fn quick_selector_focus_script_contains_mode_json() {
        let script = quick_selector_focus_script(Some(RefineMode::JsonFormat), "[]", &[]);
        assert!(script.starts_with("focusInput("));
        assert!(script.contains("JsonFormat"));
        assert!(script.ends_with(");"));
    }

    /// 自動変換が無効なときは現在モードに `null` を渡すようにすること
    #[test]
    fn quick_selector_focus_script_passes_null_without_auto_refine() {
        let script = quick_selector_focus_script(None, "[]", &[]);
        assert_eq!(script, "focusInput(null, [], []);");
    }

    /// フォーカス用スクリプトに展開中カテゴリを JSON 配列で渡すようにすること
    #[test]
    fn quick_selector_focus_script_passes_expanded_categories() {
        let script = quick_selector_focus_script(None, "[]", &["URL操作".to_string()]);
        assert_eq!(script, r#"focusInput(null, [], ["URL操作"]);"#);
    }

    /// `expanded-categories:` メッセージから展開中カテゴリを取り出せるようにすること
    #[test]
    fn parse_expanded_categories_message_reads_json_array() {
        assert_eq!(
            parse_expanded_categories_message(r#"expanded-categories:["URL操作","行操作"]"#),
            Some(vec!["URL操作".to_string(), "行操作".to_string()])
        );
        assert_eq!(
            parse_expanded_categories_message("expanded-categories:[]"),
            Some(Vec::new())
        );
        assert_eq!(
            parse_expanded_categories_message("expanded-categories:{"),
            None
        );
        assert_eq!(parse_expanded_categories_message("close"), None);
    }

    /// 上限を超える件数・長すぎるカテゴリ名は捨てるようにすること
    #[test]
    fn parse_expanded_categories_message_limits_input() {
        let long = "あ".repeat(MAX_EXPANDED_CATEGORY_CHARS + 1);
        let many: Vec<String> = (0..MAX_EXPANDED_CATEGORIES + 10)
            .map(|i| i.to_string())
            .collect();
        let msg = format!(
            "expanded-categories:{}",
            serde_json::to_string(&[vec![long], many].concat()).expect("json")
        );
        let parsed = parse_expanded_categories_message(&msg).expect("parsed");
        assert_eq!(parsed.len(), MAX_EXPANDED_CATEGORIES);
        assert_eq!(parsed[0], "0");
    }
}

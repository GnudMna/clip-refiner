use std::cell::RefCell;
use std::sync::Arc;

use super::apply::{
    SettingsApplyFeedback, SettingsWindowCommand, handle_settings_window_command,
    should_notify_apply_error_via_desktop,
};
use super::geometry::{
    apply_geometry_to_builder, apply_saved_position, load_settings_window_geometry,
    save_settings_window_geometry,
};
use super::ipc::{SettingsPatch, dispatch_settings_ipc, parse_settings_ipc_message};
use super::regex_preview::RegexPreviewResponse;
use super::snapshot::SettingsSnapshot;

use crate::consts;
use crate::tray::menu::{TrayMenu, create_window_icon, icon::app_icon_png_data_url};
use crate::tray::selector::{
    QuickSelectorWindow, WebSelectorWindow, WebViewWindowOptions, json_parse_call_script,
    settings_load_script,
};
use crate::tray::state::{AppEvent, AppState};
use crate::tray::worker::ClipboardWorkerHandle;
use crate::user_error::titles;

use anyhow::{Context, Result};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder, WindowId};

// ======================================================================
// HTML 組み立て
// ======================================================================
const SETTINGS_CSS_IMPORT: &str = r#"@import url("settings.css");"#;
const SETTINGS_JS_TAG: &str = r#"<script src="settings.js"></script>"#;
const SETTINGS_ICON_PLACEHOLDER: &str = "__APP_ICON_SRC__";
const FLUSH_PENDING_APPLY_SCRIPT: &str =
    "typeof flushPendingApply==='function'&&flushPendingApply();";
const IPC_LOG_PREVIEW_MAX_LEN: usize = 200;

/// 解釈に失敗した IPC メッセージを警告ログへ出力する
fn log_unparsed_settings_ipc(msg: &str) {
    let preview = if msg.len() > IPC_LOG_PREVIEW_MAX_LEN {
        format!("{}...", &msg[..IPC_LOG_PREVIEW_MAX_LEN])
    } else {
        msg.to_string()
    };
    crate::log_warn!("設定 IPC メッセージの解釈に失敗: {}", preview);
}

/// 設定ウィンドウ用 JavaScript を分割ファイルから組み立てる
fn assemble_settings_js() -> String {
    const FRAGMENTS: &[&str] = &[
        include_str!("../../ui/settings/regex_preview.js"),
        include_str!("../../ui/settings/settings_patch.js"),
        include_str!("../../ui/settings/shared_logic.js"),
        include_str!("../../ui/settings/panels_logic.js"),
        include_str!("../../ui/settings/shared.js"),
        include_str!("../../ui/settings/panels.js"),
        include_str!("../../ui/settings/events.js"),
        include_str!("../../ui/settings/main.js"),
    ];
    format!(
        "(function () {{\n'use strict';\n{}\n}})();",
        FRAGMENTS.join("\n")
    )
}

/// 設定ウィンドウ HTML 内のアセットをインライン展開する
fn embed_settings_assets(html: &str) -> String {
    let css = include_str!("../../ui/settings.css");
    let js = assemble_settings_js();

    html.replace(SETTINGS_CSS_IMPORT, css)
        .replace(SETTINGS_JS_TAG, &format!("<script>\n{js}\n</script>"))
        .replace(SETTINGS_ICON_PLACEHOLDER, &app_icon_png_data_url())
}

/// 設定ウィンドウ用 HTML を組み立てる
fn assemble_settings_html() -> String {
    embed_settings_assets(include_str!("../../ui/settings.html"))
}

// ======================================================================
// 設定ウィンドウ
// ======================================================================
/// 設定編集用 `WebView` ウィンドウ
///
/// `WebView` は初回表示時に生成し、非表示時に破棄してアイドル RSS を抑える
pub struct SettingsWindow {
    /// イベント送信用プロキシ
    proxy: EventLoopProxy<AppEvent>,
    /// 共有アプリケーション状態
    state: Arc<AppState>,
    /// 遅延生成された `WebView` (非表示時は `None`)
    inner: RefCell<Option<WebSelectorWindow>>,
}

impl SettingsWindow {
    /// `WebView` 未生成の空シェルを生成する
    ///
    /// # Arguments
    /// * `proxy` - イベント送信用プロキシ
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Self` - 遅延生成用の空シェル
    pub fn new_shell(proxy: EventLoopProxy<AppEvent>, state: Arc<AppState>) -> Self {
        Self {
            proxy,
            state,
            inner: RefCell::new(None),
        }
    }

    /// `WebView` を構築する
    fn build_webview(
        window: Window,
        proxy: &EventLoopProxy<AppEvent>,
        state: &Arc<AppState>,
    ) -> Result<WebSelectorWindow> {
        let html = assemble_settings_html();
        let proxy_clone = proxy.clone();
        let state_clone = Arc::clone(state);
        let data_dir = format!("{}-Settings-WebView2", consts::APP_NAME);

        WebSelectorWindow::build(
            window,
            &data_dir,
            &html,
            move |req| {
                let msg = req.body();
                if let Some(action) = parse_settings_ipc_message(msg) {
                    dispatch_settings_ipc(&proxy_clone, &state_clone, action);
                } else {
                    log_unparsed_settings_ipc(msg);
                }
            },
            WebViewWindowOptions::OPAQUE,
        )
    }

    /// 未生成なら `WebView` を生成する
    fn ensure_created(&self, event_loop: &EventLoopWindowTarget<AppEvent>) -> Result<()> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }
        let window = build_hidden_settings_window(event_loop)?;
        let webview = Self::build_webview(window, &self.proxy, &self.state)?;
        *self.inner.borrow_mut() = Some(webview);
        Ok(())
    }

    /// 設定ウィンドウを表示し、スナップショットを読み込む
    ///
    /// 未生成なら `WebView` を遅延生成する
    ///
    /// # Arguments
    /// * `event_loop` - 遅延生成用のイベントループ
    /// * `snapshot_json` - 設定スナップショット JSON
    pub fn show(&self, event_loop: &EventLoopWindowTarget<AppEvent>, snapshot_json: &str) {
        if let Err(err) = self.ensure_created(event_loop) {
            crate::log_warn!("設定ウィンドウの初期化に失敗: {err:#}");
            crate::platform::show_notification("設定", "ウィンドウの初期化に失敗しました");
            return;
        }
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        inner.window().set_visible(true);
        inner.window().set_focus();
        let script = settings_load_script(snapshot_json, false, false);
        inner.evaluate_script(&script);
    }

    /// 前面へフォーカスを移す
    ///
    /// # Arguments
    /// * `self` - この `SettingsWindow`
    pub fn focus(&self) {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.window().set_focus();
        }
    }

    /// JavaScript へスナップショットを渡す
    ///
    /// # Arguments
    /// * `snapshot_json` - 設定スナップショット JSON
    pub fn load_snapshot(&self, snapshot_json: &str) {
        self.load_snapshot_with_options(snapshot_json, false, false);
    }

    /// JavaScript へスナップショットを渡す (`force` 時は編集中でも UI を再描画する)
    ///
    /// # Arguments
    /// * `snapshot_json` - 設定スナップショット JSON
    /// * `force` - 編集中でも強制的に UI を再描画するか
    /// * `external_sync` - 外部同期由来の更新かどうか
    pub fn load_snapshot_with_options(
        &self,
        snapshot_json: &str,
        force: bool,
        external_sync: bool,
    ) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let script = settings_load_script(snapshot_json, force, external_sync);
        inner.evaluate_script(&script);
    }

    /// 登録クリップ一覧のみ JavaScript へ反映する
    pub(crate) fn update_clips(&self, clips_json: &str) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let script = json_parse_call_script("updateClips", clips_json);
        inner.evaluate_script(&script);
    }

    /// お気に入りとショートカット一覧のみ JavaScript へ反映する
    pub(crate) fn update_favorites_and_shortcuts(&self, json: &str) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let script = json_parse_call_script("updateFavoritesAndShortcuts", json);
        inner.evaluate_script(&script);
    }

    /// debounce 中の設定変更を即時送信する
    ///
    /// # Arguments
    /// * `self` - この `SettingsWindow`
    pub fn flush_pending_apply(&self) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        inner.evaluate_script(FLUSH_PENDING_APPLY_SCRIPT);
    }

    /// 設定適用結果を JavaScript へ通知する
    ///
    /// ウィンドウ非表示時は適用エラーのみデスクトップ通知へフォールバックする
    pub(crate) fn notify_apply_feedback(&self, feedback: &SettingsApplyFeedback) {
        if self.is_visible() {
            let Ok(json) = serde_json::to_string(feedback) else {
                return;
            };
            let script = json_parse_call_script("onApplyFeedback", &json);
            if let Some(inner) = self.inner.borrow().as_ref() {
                inner.evaluate_script(&script);
            }
            return;
        }

        if should_notify_apply_error_via_desktop(self.is_visible(), feedback) {
            crate::platform::show_notification(titles::SETTINGS, &feedback.message);
        }
    }

    /// 正規表現プレビュー結果を JavaScript へ通知する
    pub(crate) fn notify_regex_preview(&self, response: &RegexPreviewResponse) {
        if !self.is_visible() {
            return;
        }
        let Ok(json) = serde_json::to_string(response) else {
            return;
        };
        let script = json_parse_call_script("onRegexPreview", &json);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.evaluate_script(&script);
        }
    }

    /// 設定ウィンドウを非表示にし、`WebView` を破棄する
    ///
    /// # Arguments
    /// * `self` - この `SettingsWindow`
    pub fn hide(&self) {
        self.flush_pending_apply();
        {
            let guard = self.inner.borrow();
            if let Some(inner) = guard.as_ref() {
                save_settings_window_geometry(inner.window());
            }
        }
        *self.inner.borrow_mut() = None;
    }

    /// OS の閉じる操作向けに JS の `closeWindow` を実行する
    ///
    /// デバウンス中パッチを IPC 経由で先に送ってから非表示にする
    pub(crate) fn request_close_from_os(&self) {
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        inner.evaluate_script("typeof closeWindow==='function'&&closeWindow();");
    }

    /// 設定ウィンドウが表示中かどうか
    ///
    /// # Arguments
    /// * `self` - この `SettingsWindow`
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        self.inner
            .borrow()
            .as_ref()
            .is_some_and(WebSelectorWindow::is_visible)
    }

    /// ウィンドウ ID を返す
    ///
    /// # Arguments
    /// * `self` - この `SettingsWindow`
    ///
    /// # Returns
    /// * `Option<WindowId>` - 生成済みならウィンドウ ID、未生成なら `None`
    pub fn id(&self) -> Option<WindowId> {
        self.inner.borrow().as_ref().map(WebSelectorWindow::id)
    }

    /// 設定ウィンドウ関連の `AppEvent` を処理する
    pub(crate) fn handle_event(
        &self,
        event: AppEvent,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        state: &Arc<AppState>,
        menu: &TrayMenu,
        quick_selector: Option<&QuickSelectorWindow>,
        clipboard_worker: &ClipboardWorkerHandle,
    ) {
        match event {
            AppEvent::ShowSettingsWindow => open_settings_window(event_loop, state, self),
            AppEvent::HideSettingsWindow => self.hide(),
            AppEvent::SettingsApplyPatch => {
                if let Some(json) = state.take_pending_settings_patch() {
                    match serde_json::from_str::<SettingsPatch>(&json) {
                        Ok(patch) => handle_settings_window_command(
                            SettingsWindowCommand::Apply(Box::new(patch)),
                            state,
                            menu,
                            self,
                            quick_selector,
                            clipboard_worker,
                        ),
                        Err(err) => {
                            crate::log_warn!("設定パッチの JSON 解釈に失敗: {:?}", err);
                            self.notify_apply_feedback(&SettingsApplyFeedback::error(
                                None,
                                "設定の適用に失敗しました",
                            ));
                        }
                    }
                }
            }
            AppEvent::SettingsClearHistory => {
                handle_settings_window_command(
                    SettingsWindowCommand::ClearHistory,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsClearContinuousRecord => {
                handle_settings_window_command(
                    SettingsWindowCommand::ClearContinuousRecord,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsOpenConfigFile => {
                handle_settings_window_command(
                    SettingsWindowCommand::OpenConfigFile,
                    state,
                    menu,
                    self,
                    quick_selector,
                    clipboard_worker,
                );
            }
            AppEvent::SettingsRegexPreview => {
                if let Some(request) = state.take_regex_preview_request() {
                    let response = RegexPreviewResponse::from_request(&request);
                    self.notify_regex_preview(&response);
                }
            }
            _ => {}
        }
    }
}

// ======================================================================
// ウィンドウ生成
// ======================================================================
/// 画面中央に配置する非表示の設定ウィンドウを生成する
fn build_hidden_settings_window(event_loop: &EventLoopWindowTarget<AppEvent>) -> Result<Window> {
    let geometry = load_settings_window_geometry();
    let builder = WindowBuilder::new()
        .with_title(format!("{} 設定", consts::APP_NAME))
        .with_window_icon(Some(
            create_window_icon().context("ウィンドウアイコンの読み込みに失敗")?,
        ))
        .with_visible(false)
        .with_resizable(true)
        .with_min_inner_size(tao::dpi::LogicalSize::new(640.0, 480.0));
    let window = apply_geometry_to_builder(builder, geometry).build(event_loop)?;
    apply_saved_position(&window, geometry);
    Ok(window)
}

/// 設定ウィンドウの空シェルを初期化する (`WebView` は初回表示まで作らない)
///
/// # Arguments
/// * `proxy` - イベント送信用プロキシ
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `SettingsWindow` - 遅延生成用シェル
pub fn init_settings_window(
    proxy: &EventLoopProxy<AppEvent>,
    state: &Arc<AppState>,
) -> SettingsWindow {
    SettingsWindow::new_shell(proxy.clone(), Arc::clone(state))
}

/// 設定ウィンドウを開く (既に表示中なら前面へ)
pub(crate) fn open_settings_window(
    event_loop: &EventLoopWindowTarget<AppEvent>,
    state: &Arc<AppState>,
    settings: &SettingsWindow,
) {
    let snapshot = state.with_config(SettingsSnapshot::from_config);
    if settings.is_visible() {
        settings.load_snapshot(&snapshot.to_json());
        settings.focus();
    } else {
        settings.show(event_loop, &snapshot.to_json());
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{SettingsWindow, assemble_settings_html, assemble_settings_js};

    use crate::tray::state::test_app_state;

    /// 空シェルは非表示でウィンドウ ID もなく、未生成時の操作が no-op であること
    #[test]
    fn empty_shell_lifecycle_is_safe() {
        let state = Arc::new(test_app_state());
        let settings = SettingsWindow::new_shell(state.proxy.clone(), Arc::clone(&state));

        assert!(!settings.is_visible());
        assert!(settings.id().is_none());
        settings.load_snapshot("{}");
        settings.focus();
        settings.flush_pending_apply();
        settings.hide();
        assert!(!settings.is_visible());
        assert!(settings.id().is_none());
    }

    /// 設定 UI JavaScript に正規表現プレビューの stale 判定ロジックが含まれるようにすること
    #[test]
    fn assembled_settings_js_includes_regex_preview_stale_guard() {
        let js = assemble_settings_js();
        assert!(js.contains("resolveRegexPreviewResponse"));
        assert!(js.contains("pendingRegexPreviewRequest"));
        assert!(js.contains("action: 'stale'"));
    }

    /// 設定 UI JavaScript に共有純粋ロジックが含まれるようにすること
    #[test]
    fn assembled_settings_js_includes_shared_logic() {
        let js = assemble_settings_js();
        assert!(js.contains("SettingsSharedLogic"));
        assert!(js.contains("SettingsPanelsLogic"));
        assert!(js.contains("buildNotificationSettingsPatch"));
        assert!(js.contains("normalizePipeline"));
        assert!(js.contains("shouldFullRenderOnLoad"));
        assert!(js.contains("applyPipelineAction"));
        assert!(js.contains("resolveContentClickAction"));
    }

    /// 組み立て HTML にシェル必須 DOM と全タブ定義が含まれるようにすること
    #[test]
    fn assembled_settings_html_includes_shell_and_tabs() {
        let html = assemble_settings_html();
        for marker in [
            r#"id="app""#,
            r#"id="sidebar""#,
            r#"id="content""#,
            r#"id="open-config-btn""#,
            r#"id="close-btn""#,
            r#"id="apply-feedback""#,
            r#"id="confirm-dialog""#,
        ] {
            assert!(
                html.contains(marker),
                "組み立て HTML に {marker} が含まれること"
            );
        }

        let js = assemble_settings_js();
        for tab_id in [
            "general",
            "favorites",
            "regex",
            "history",
            "clips",
            "monitor",
            "notifications",
            "shortcuts",
        ] {
            let marker = format!("id: '{tab_id}'");
            assert!(
                js.contains(&marker),
                "組み立て JS にタブ {tab_id} が含まれること"
            );
        }
    }

    /// CSS / JS がインライン展開され外部参照が残らないようにすること
    #[test]
    fn assembled_settings_html_inlines_assets() {
        let html = assemble_settings_html();
        assert!(!html.contains(r#"@import url("settings.css")"#));
        assert!(!html.contains(r#"<script src="settings.js"></script>"#));
        assert!(!html.contains("__APP_ICON_SRC__"));
        assert!(html.contains("<script>"));
        assert!(html.contains("SettingsSharedLogic"));
    }
}

//! 設定編集用 `WebView` ウィンドウ
//!
//! HTML/JS 組み立ては `assets`、`SettingsEvent` 処理は `events` に分離する

mod assets;
mod events;

use std::cell::RefCell;
use std::sync::Arc;

use anyhow::{Context, Result};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder, WindowId};

use super::apply::{SettingsApplyFeedback, should_notify_apply_error_via_desktop};
use super::geometry::{
    apply_geometry_to_builder, apply_saved_position, load_settings_window_geometry,
    save_settings_window_geometry,
};
use super::ipc::{dispatch_settings_ipc, parse_settings_ipc_message};
use super::regex_preview::RegexPreviewResponse;
use super::snapshot::SettingsSnapshot;
use crate::consts;
use crate::platform::update::{UpdateApplyUiResponse, UpdateCheckUiResponse};
use crate::tray::menu::create_window_icon;
use crate::tray::selector::{
    WebSelectorWindow, WebViewWindowOptions, json_parse_call_script, settings_load_script,
};
use crate::tray::state::{AppEvent, AppState};
use crate::user_error::titles;
use assets::{
    CLOSE_FROM_OS_SCRIPT, FLUSH_PENDING_APPLY_SCRIPT, assemble_settings_html,
    log_unparsed_settings_ipc,
};

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
            crate::user_error::notify_webview_window_init_failure("設定", &err);
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

    /// 更新確認結果を JavaScript へ通知する
    pub(crate) fn notify_update_check(&self, response: &UpdateCheckUiResponse) {
        if !self.is_visible() {
            return;
        }
        let Ok(json) = serde_json::to_string(response) else {
            return;
        };
        let script = json_parse_call_script("onUpdateCheck", &json);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.evaluate_script(&script);
        }
    }

    /// 更新適用結果を JavaScript へ通知する
    pub(crate) fn notify_update_apply(&self, response: &UpdateApplyUiResponse) {
        if !self.is_visible() {
            return;
        }
        let Ok(json) = serde_json::to_string(response) else {
            return;
        };
        let script = json_parse_call_script("onUpdateApply", &json);
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
        inner.evaluate_script(CLOSE_FROM_OS_SCRIPT);
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
    crate::tray::e2e::signal_ready();
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::SettingsWindow;
    use crate::tray::state::test_app_state;

    /// 空シェルは非表示でウィンドウ ID もなく、未生成時の操作が no-op となるようにすること
    #[test]
    fn empty_shell_lifecycle_is_safe() {
        let state = Arc::new(test_app_state());
        let settings = SettingsWindow::new_shell(state.proxy.clone(), Arc::clone(&state));

        assert!(!settings.is_visible());
        assert!(settings.id().is_none());
        settings.load_snapshot("{}");
        settings.focus();
        settings.flush_pending_apply();
        settings.request_close_from_os();
        settings.hide();
        assert!(!settings.is_visible());
        assert!(settings.id().is_none());
    }
}

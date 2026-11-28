//! セレクターウィンドウ共通の遅延生成 `WebView` 管理

use std::cell::{Cell, RefCell};
use std::time::Instant;

use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowId};

use super::retention::RetainedSelector;
use super::window::{WebSelectorWindow, build_hidden_selector_window};
use crate::tray::state::AppEvent;

// ======================================================================
// 定数
// ======================================================================
/// セレクターウィンドウの幅
const SELECTOR_WINDOW_WIDTH: f64 = 520.0;
/// セレクターウィンドウの高さ
const SELECTOR_WINDOW_HEIGHT: f64 = 600.0;

// ======================================================================
// 遅延生成 WebView
// ======================================================================
/// 初回表示時に生成する `WebView` と、その保持方針用の状態を束ねる構造体
///
/// 非表示後の保持 / 破棄は `selector_webview_retention` 設定に従う
pub(crate) struct LazySelector {
    /// 遅延生成された `WebView` (未生成または破棄後は `None`)
    inner: RefCell<Option<WebSelectorWindow>>,
    /// `DisposeAfterIdle` 用の非表示開始時刻
    idle_hidden_at: Cell<Option<Instant>>,
    /// テスト用の擬似表示フラグ (`WebView` なしで相互排他を検証する)
    #[cfg(test)]
    force_visible: Cell<bool>,
}

impl LazySelector {
    /// `WebView` 未生成の空シェルを生成する
    ///
    /// # Returns
    /// * `Self` - 遅延生成用の空シェル
    pub(crate) fn new() -> Self {
        Self {
            inner: RefCell::new(None),
            idle_hidden_at: Cell::new(None),
            #[cfg(test)]
            force_visible: Cell::new(false),
        }
    }

    /// 未生成なら `WebView` を生成する
    ///
    /// # Arguments
    /// * `event_loop` - ウィンドウ作成用のイベントループ
    /// * `title` - ウィンドウタイトル
    /// * `build` - 非表示ウィンドウから `WebView` を構築する処理
    ///
    /// # Returns
    /// * `anyhow::Result<()>` - 生成または既存利用に成功した場合は `Ok(())`
    fn ensure_created(
        &self,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        title: &str,
        build: impl FnOnce(Window) -> anyhow::Result<WebSelectorWindow>,
    ) -> anyhow::Result<()> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }
        let window = build_hidden_selector_window(
            event_loop,
            title,
            SELECTOR_WINDOW_WIDTH,
            SELECTOR_WINDOW_HEIGHT,
        )?;
        *self.inner.borrow_mut() = Some(build(window)?);
        Ok(())
    }

    /// `WebView` を (必要なら生成して) 表示し、スクリプトを実行する
    ///
    /// 表示を試みた時点で `DisposeAfterIdle` の期限はリセットする
    ///
    /// # Arguments
    /// * `event_loop` - 遅延生成用のイベントループ
    /// * `title` - ウィンドウタイトル
    /// * `build` - 非表示ウィンドウから `WebView` を構築する処理
    /// * `script` - 表示時に実行するスクリプト
    ///
    /// # Returns
    /// * `anyhow::Result<()>` - `WebView` の生成に失敗した場合は `Err`
    pub(crate) fn show(
        &self,
        event_loop: &EventLoopWindowTarget<AppEvent>,
        title: &str,
        build: impl FnOnce(Window) -> anyhow::Result<WebSelectorWindow>,
        script: &str,
    ) -> anyhow::Result<()> {
        self.idle_hidden_at.set(None);
        self.ensure_created(event_loop, title, build)?;
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.show_with_script(script);
        }
        Ok(())
    }

    /// 生成済みの `WebView` でスクリプトを実行する (未生成なら何もしない)
    ///
    /// # Arguments
    /// * `script` - 実行するスクリプト
    pub(crate) fn evaluate_script(&self, script: &str) {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.evaluate_script(script);
        }
    }

    /// ウィンドウを非表示にする (`WebView` の破棄は呼び出し側の保持方針に任せる)
    pub(crate) fn hide(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.hide();
        }
    }

    /// `WebView` を破棄する (未生成なら何もしない)
    pub(crate) fn dispose(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        self.idle_hidden_at.set(None);
        *self.inner.borrow_mut() = None;
    }

    /// プロセスの異常終了で使えなくなった `WebView` を破棄する
    ///
    /// 次の表示で作り直す
    ///
    /// # Arguments
    /// * `window_id` - 異常終了を通知したウィンドウ ID
    ///
    /// # Returns
    /// * `Option<bool>` - 破棄した場合は破棄前に表示中だったか。別のウィンドウなら `None`
    pub(crate) fn dispose_failed(&self, window_id: WindowId) -> Option<bool> {
        if self.id() != Some(window_id) {
            return None;
        }
        let was_visible = self.is_visible();
        self.dispose();
        Some(was_visible)
    }

    /// `WebView` が生成済みかどうか
    ///
    /// # Returns
    /// * `bool` - 生成済みなら `true`
    pub(crate) fn is_created(&self) -> bool {
        self.inner.borrow().is_some()
    }

    /// 表示中かどうか
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub(crate) fn is_visible(&self) -> bool {
        #[cfg(test)]
        if self.force_visible.get() {
            return true;
        }
        self.inner
            .borrow()
            .as_ref()
            .is_some_and(WebSelectorWindow::is_visible)
    }

    /// ウィンドウの内部 ID を取得する
    ///
    /// # Returns
    /// * `Option<WindowId>` - 生成済みならウィンドウ ID、未生成なら `None`
    pub(crate) fn id(&self) -> Option<WindowId> {
        self.inner.borrow().as_ref().map(WebSelectorWindow::id)
    }

    /// テスト用に表示中として扱う (実 `WebView` は作らない)
    ///
    /// # Arguments
    /// * `visible` - 擬似表示するか
    #[cfg(test)]
    pub(crate) fn force_visible_for_test(&self, visible: bool) {
        self.force_visible.set(visible);
    }
}

// ======================================================================
// 保持方針への接続
// ======================================================================
/// `LazySelector` を内包するセレクターウィンドウ
///
/// 実装すると `RetainedSelector` を自動で満たす
pub(crate) trait HasLazySelector {
    /// 内包する `LazySelector`
    fn lazy(&self) -> &LazySelector;
}

impl<T: HasLazySelector> RetainedSelector for T {
    fn hide_visual(&self) {
        self.lazy().hide();
    }

    fn dispose_webview(&self) {
        self.lazy().dispose();
    }

    fn is_visible(&self) -> bool {
        self.lazy().is_visible()
    }

    fn is_created(&self) -> bool {
        self.lazy().is_created()
    }

    fn idle_hidden_at(&self) -> Option<Instant> {
        self.lazy().idle_hidden_at.get()
    }

    fn set_idle_hidden_at(&self, at: Option<Instant>) {
        self.lazy().idle_hidden_at.set(at);
    }
}

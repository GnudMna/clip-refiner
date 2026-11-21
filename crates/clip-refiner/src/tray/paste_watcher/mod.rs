//! 貼り付け操作を検知し、連続記録キュー先頭を貼り付ける

mod hook;
mod paste;

use std::sync::Arc;

use super::state::AppState;
use super::worker::ClipboardWorkerHandle;

use tao::event_loop::EventLoopProxy;

use super::state::AppEvent;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use paste::try_paste_continuous_record_front;

// ======================================================================
// 貼り付け監視
// ======================================================================
/// 貼り付けキー入力を監視し、連続記録キュー先頭の貼り付けを行う
pub struct PasteWatcher {
    _stop: hook::PasteWatcherStop,
}

impl PasteWatcher {
    /// 貼り付け監視を開始する
    ///
    /// # Arguments
    /// * `proxy` - イベント送信用プロキシ
    /// * `state` - アプリケーションの共有状態
    /// * `clipboard_worker` - クリップボード操作ワーカー
    ///
    /// # Returns
    /// * `Self` - 貼り付け監視インスタンス
    pub fn start(
        proxy: EventLoopProxy<AppEvent>,
        state: Arc<AppState>,
        clipboard_worker: Arc<ClipboardWorkerHandle>,
    ) -> Self {
        let stop = hook::start(proxy, state, clipboard_worker);
        Self { _stop: stop }
    }
}

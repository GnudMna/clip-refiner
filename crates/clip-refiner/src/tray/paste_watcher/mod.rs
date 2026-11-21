//! 貼り付け操作を検知し、連続記録キュー先頭を貼り付ける
//!
//! 連続記録キューがあるときだけ `Ctrl+V` を `RegisterHotKey` で捕捉し、
//! クリップボード差し替え後に `SendInput` で貼り付けを通す。
//! 全キーを見る低レベルキーボードフックは使わない。

mod hotkey;
mod paste;

use std::sync::Arc;

use super::state::AppState;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use paste::try_paste_continuous_record_front;

// ======================================================================
// 貼り付け監視
// ======================================================================
/// 貼り付けキー入力を監視し、連続記録キュー先頭の貼り付けを行う
pub struct PasteWatcher {
    _stop: hotkey::PasteWatcherStop,
}

impl PasteWatcher {
    /// 貼り付け監視を開始する
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Self` - 貼り付け監視インスタンス
    pub fn start(state: Arc<AppState>) -> Self {
        let stop = hotkey::start(state);
        Self { _stop: stop }
    }
}

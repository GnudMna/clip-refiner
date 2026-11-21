use serde::{Deserialize, Serialize};

// ======================================================================
// セレクター WebView 保持方針
// ======================================================================
/// クイック / 登録クリップセレクターの `WebView` 保持方針
///
/// 既定は `Keep` (非表示後も保持して再表示を高速化)
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectorWebviewRetention {
    /// 非表示後も `WebView` を保持する
    #[default]
    Keep,
    /// 非表示時に `WebView` を破棄する
    DisposeOnHide,
    /// 非表示から一定時間経過後に `WebView` を破棄する
    DisposeAfterIdle,
}

/// アイドル破棄までの既定分数を返す
///
/// # Returns
/// * `u64` - 既定のアイドル分数
pub fn default_selector_webview_idle_minutes() -> u64 {
    crate::consts::DEFAULT_SELECTOR_WEBVIEW_IDLE_MINUTES
}

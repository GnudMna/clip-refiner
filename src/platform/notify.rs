use crate::consts;

/// 通知タイトル用のサマリー文字列を組み立てる
pub(crate) fn format_notification_summary(summary: &str) -> String {
    format!("{} - {}", consts::APP_NAME, summary)
}

// ======================================================================
// 非 Windows 実装
// ======================================================================
#[cfg(not(windows))]
mod platform_impl {
    use std::time::Duration;

    use super::format_notification_summary;

    use notify_rust::Notification;

    /// システム通知を表示する
    ///
    /// OSの通知機能を使用して、デスクトップ上にメッセージを表示する
    /// 通知は約3秒後に自動的に消える
    /// 表示に失敗した場合はログへ記録する
    ///
    /// # Arguments
    /// * `summary` - 通知のタイトル(「ClipRefiner - タイトル」の形式で表示される)
    /// * `body` - 通知の本文
    pub fn show_notification(summary: &str, body: &str) {
        if let Err(e) = Notification::new()
            .summary(&format_notification_summary(summary))
            .body(body)
            .timeout(Duration::from_secs(3))
            .show()
        {
            crate::log_warn!("通知の表示に失敗: {:?}", e);
        }
    }
}

#[cfg(not(windows))]
pub use platform_impl::show_notification;

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// サマリーにアプリ名プレフィックスが付くこと
    #[test]
    fn format_notification_summary_includes_app_name() {
        assert_eq!(
            format_notification_summary("変換完了"),
            "ClipRefiner - 変換完了"
        );
    }
}

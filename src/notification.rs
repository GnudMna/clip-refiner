use std::time::Duration;

use crate::consts;

use notify_rust::Notification;

// ======================================================================
// 通知表示
// ======================================================================
/// システム通知を表示する
///
/// OSの通知機能を使用して、デスクトップ上にメッセージを表示する
/// 通知は約3秒後に自動的に消える
/// 表示に失敗した場合はログへ記録する
///
/// # Arguments
/// * `summary` - 通知のタイトル（「ClipRefiner - タイトル」の形式で表示される）
/// * `body` - 通知の本文
pub fn show_notification(summary: &str, body: &str) {
    if let Err(e) = Notification::new()
        .summary(&format!("{} - {}", consts::APP_NAME, summary))
        .body(body)
        .timeout(Duration::from_secs(3))
        .show()
    {
        crate::log_warn!("通知の表示に失敗: {:?}", e);
    }
}

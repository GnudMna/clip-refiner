use std::time::Duration;

use crate::consts;

use notify_rust::Notification;

// ======================================================================
// 通知表示
// ======================================================================
/// システム通知を表示する
///
/// OSの通知機能を使用して、デスクトップ上にメッセージを表示します。
/// 通知は約3秒後に自動的に消えます。
///
/// # Arguments
/// * `summary` - 通知のタイトル（「ClipRefiner - タイトル」の形式で表示されます）
/// * `body` - 通知の本文
pub fn show_notification(summary: &str, body: &str) {
    let _ = Notification::new()
        .summary(&format!("{} - {}", consts::APP_NAME, summary))
        .body(body)
        .timeout(Duration::from_secs(3))
        .show();
}

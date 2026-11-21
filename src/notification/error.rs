use std::time::Duration;

use notify_rust::Notification;

/// エラー通知を表示する
pub fn show_error_notification(summary: &str, body: &str) {
    let _ = Notification::new()
        .summary(&format!("ClipCoder - {}", summary))
        .body(body)
        .timeout(Duration::from_secs(5))
        .show();
}

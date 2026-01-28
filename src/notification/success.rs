#[cfg(debug_assertions)]
use notify_rust::Notification;
#[cfg(debug_assertions)]
use std::time::Duration;

/// 成功通知を表示する
#[cfg(debug_assertions)]
pub fn show_success_notification(summary: &str, body: &str) {
    let _ = Notification::new()
        .summary(&format!("ClipRefiner - {}", summary))
        .body(body)
        .timeout(Duration::from_secs(3))
        .show();
}

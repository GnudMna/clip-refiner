use std::time::Duration;

use notify_rust::Notification;

/// エラー通知を表示する
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `body` - 通知の本文。
pub fn show_error_notification(summary: &str, body: &str) {
    let _ = Notification::new()
        .summary(&format!("ClipRefiner - {}", summary))
        .body(body)
        .timeout(Duration::from_secs(5))
        .show();
}

/// anyhow::Error からエラー通知を表示する
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `err` - 表示する `anyhow::Error` インスタンス。
pub fn show_anyhow_error(summary: &str, err: &anyhow::Error) {
    let body = format!("{:#}", err); // {:#} で原因のチェーンを含めて表示
    show_error_notification(summary, &body);
}

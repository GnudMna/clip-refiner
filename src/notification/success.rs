/// 成功通知を表示する(デバッグビルドのみ)
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `body` - 通知の本文。
pub fn show_success_debug_notification(_summary: &str, _body: &str) {
    #[cfg(debug_assertions)]
    {
        use std::time::Duration;

        use notify_rust::Notification;

        let _ = Notification::new()
            .summary(&format!("ClipRefiner - {}", _summary))
            .body(_body)
            .timeout(Duration::from_secs(3))
            .show();
    }
}

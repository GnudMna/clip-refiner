use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::notification;
use crate::refiner::RefineMode;
use crate::tray::state::AppState;

/// 変換完了通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの状態（通知設定を含む）
/// * `mode` - 実行された変換モード
/// * `processed` - 変換後のテキスト
pub fn show_process_notification(state: &Arc<AppState>, mode: RefineMode, processed: &str) {
    if !state.show_success_notification.load(Ordering::Relaxed) {
        return;
    }

    let mut lines = Vec::new();
    if state.notification_notify_mode.load(Ordering::Relaxed) {
        lines.push(format!("モード: {}", mode.label()));
    }
    if state.notification_notify_result.load(Ordering::Relaxed) {
        let snippet = if processed.chars().count() > 50 {
            format!("{}...", processed.chars().take(47).collect::<String>())
        } else {
            processed.to_string()
        };
        lines.push(format!("内容: {}", snippet));
    }
    notification::show_notification("変換完了", &lines.join("\n"));
}

/// 一時停止/再開通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの状態（通知設定を含む）
/// * `paused` - 一時停止状態かどうか（true: 一時停止、false: 再開）
/// * `source` - 通知のタイトル（例: "ショートカット"、"設定変更"）
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    if state.show_success_notification.load(Ordering::Relaxed)
        && state.notification_notify_pause.load(Ordering::Relaxed)
    {
        notification::show_notification(
            source,
            if paused {
                "クリップボード監視を一時停止しました"
            } else {
                "クリップボード監視を再開しました"
            },
        );
    }
}

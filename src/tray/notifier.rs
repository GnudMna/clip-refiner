use std::sync::Arc;

use crate::notification;
use crate::refiner::RefineMode;
use crate::tray::state::AppState;

/// 加工完了時のデスクトップ通知を表示する
///
/// 設定に応じて、実行されたモード名や加工後のテキストスニペットを通知内容に含めます。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態（通知設定の参照用）
/// * `mode` - 実行された加工モード
/// * `processed` - 加工後のテキスト
pub fn show_process_notification(state: &Arc<AppState>, mode: RefineMode, processed: &str) {
    if !state.show_success_notification() {
        return;
    }

    let mut lines = Vec::new();
    if state.notify_mode() {
        lines.push(format!("モード: {}", mode.label()));
    }
    if state.notify_result() {
        let snippet = if processed.chars().count() > 50 {
            format!("{}...", processed.chars().take(47).collect::<String>())
        } else {
            processed.to_string()
        };
        lines.push(format!("内容: {}", snippet));
    }
    notification::show_notification("変換完了", &lines.join("\n"));
}

/// 監視の一時停止または再開時のデスクトップ通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `paused` - 新しい一時停止状態 (`true`: 一時停止中, `false`: 監視中)
/// * `source` - 通知のタイトル（操作元を示す文字列、例: "ショートカット", "設定変更"）
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    if state.show_success_notification() && state.notify_pause() {
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

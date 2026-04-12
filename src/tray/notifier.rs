use std::sync::Arc;

use crate::notification;
use crate::refiner::RefineMode;
use crate::tray::state::AppState;

fn make_result_snippet(processed: &str) -> String {
    if processed.chars().count() > 50 {
        format!("{}...", processed.chars().take(47).collect::<String>())
    } else {
        processed.to_string()
    }
}

fn build_process_notification_body(
    mode: RefineMode,
    processed: &str,
    notify_mode: bool,
    notify_result: bool,
) -> String {
    let mut lines = Vec::new();
    if notify_mode {
        lines.push(format!("モード: {}", mode.label()));
    }
    if notify_result {
        lines.push(format!("内容: {}", make_result_snippet(processed)));
    }
    lines.join("\n")
}

fn pause_message(paused: bool) -> &'static str {
    if paused {
        "クリップボード監視を一時停止しました"
    } else {
        "クリップボード監視を再開しました"
    }
}

// ======================================================================
// 加工完了通知
// ======================================================================
/// 加工完了時のデスクトップ通知を表示する
///
/// 設定に応じて、実行されたモード名や加工後のテキストスニペットを通知内容に含めます。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態（通知設定の参照用）
/// * `mode` - 実行された加工モード
/// * `processed` - 加工後のテキスト
pub fn show_process_notification(state: &Arc<AppState>, mode: RefineMode, processed: &str) {
    if !state.is_notification_enabled() {
        return;
    }

    let body = build_process_notification_body(
        mode,
        processed,
        state.notify_mode(),
        state.notify_result(),
    );
    notification::show_notification("変換完了", &body);
}

// ======================================================================
// 一時停止通知
// ======================================================================
/// 監視の一時停止または再開時のデスクトップ通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `paused` - 新しい一時停止状態 (`true`: 一時停止中, `false`: 監視中)
/// * `source` - 通知のタイトル（操作元を示す文字列、例: "ショートカット", "設定変更"）
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    if state.is_notification_enabled() && state.notify_pause() {
        notification::show_notification(source, pause_message(paused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_result_snippet_short() {
        assert_eq!(make_result_snippet("short text"), "short text");
    }

    #[test]
    fn test_make_result_snippet_long() {
        let input = "x".repeat(60);
        let out = make_result_snippet(&input);
        assert_eq!(out.chars().count(), 50);
        assert!(out.ends_with("..."));
    }

    #[test]
    fn test_build_process_notification_body_mode_and_result() {
        let body = build_process_notification_body(RefineMode::Trim, "abc", true, true);
        assert!(body.contains("モード: 全体をトリム"));
        assert!(body.contains("内容: abc"));
        assert!(body.contains('\n'));
    }

    #[test]
    fn test_build_process_notification_body_mode_only() {
        let body = build_process_notification_body(RefineMode::Trim, "abc", true, false);
        assert_eq!(body, "モード: 全体をトリム");
    }

    #[test]
    fn test_build_process_notification_body_result_only() {
        let body = build_process_notification_body(RefineMode::Trim, "abc", false, true);
        assert_eq!(body, "内容: abc");
    }

    #[test]
    fn test_build_process_notification_body_empty() {
        let body = build_process_notification_body(RefineMode::Trim, "abc", false, false);
        assert!(body.is_empty());
    }

    #[test]
    fn test_pause_message() {
        assert_eq!(pause_message(true), "クリップボード監視を一時停止しました");
        assert_eq!(pause_message(false), "クリップボード監視を再開しました");
    }
}

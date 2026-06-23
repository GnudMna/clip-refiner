use std::sync::Arc;

use crate::config::NotificationSettings;
use crate::notification;
use crate::refiner::RefineMode;
use crate::tray::state::AppState;

// ======================================================================
// 加工完了通知
// ======================================================================
/// 加工完了通知の本文を組み立てる
///
/// 通知が無効、または表示する行がない場合は `None` を返す
///
/// # Arguments
/// * `settings` - 通知設定
/// * `mode` - 実行された加工モード
/// * `processed` - 加工後のテキスト
///
/// # Returns
/// * `Option<String>` - 通知本文。表示不要な場合は `None`
pub(crate) fn format_process_notification_body(
    settings: &NotificationSettings,
    mode: RefineMode,
    processed: &str,
) -> Option<String> {
    if !settings.enabled {
        return None;
    }

    let mut lines = Vec::new();
    if settings.notify_mode {
        lines.push(format!("モード: {}", mode.label()));
    }
    if settings.notify_result {
        lines.push(format!(
            "内容: {}",
            truncate_notification_snippet(processed)
        ));
    }

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
}

/// 通知用に加工結果テキストを切り詰める
///
/// 50 文字を超える場合は先頭 47 文字に `...` を付与する
fn truncate_notification_snippet(processed: &str) -> String {
    if processed.chars().count() > 50 {
        format!("{}...", processed.chars().take(47).collect::<String>())
    } else {
        processed.to_string()
    }
}

/// 加工完了時のデスクトップ通知を表示する
///
/// 設定に応じて、実行されたモード名や加工後のテキストスニペットを通知内容に含める
///
/// # Arguments
/// * `state` - アプリケーションの共有状態(通知設定の参照用)
/// * `mode` - 実行された加工モード
/// * `processed` - 加工後のテキスト
pub fn show_process_notification(state: &Arc<AppState>, mode: RefineMode, processed: &str) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    let Some(body) = format_process_notification_body(&settings, mode, processed) else {
        return;
    };
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
/// * `source` - 通知のタイトル(操作元を示す文字列、例: "ショートカット", "設定変更")
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if settings.enabled && settings.notify_pause {
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotificationSettings;

    fn enabled_settings() -> NotificationSettings {
        NotificationSettings {
            enabled: true,
            notify_mode: true,
            notify_result: true,
            notify_pause: true,
        }
    }

    /// 通知無効時は本文を生成しないこと
    #[test]
    fn format_body_returns_none_when_disabled() {
        let settings = NotificationSettings::default();
        assert!(format_process_notification_body(&settings, RefineMode::Trim, "abc").is_none());
    }

    /// モード名と加工結果の両方を含むこと
    #[test]
    fn format_body_includes_mode_and_result() {
        let body =
            format_process_notification_body(&enabled_settings(), RefineMode::UrlEncode, "hello")
                .expect("通知本文が生成される");

        assert!(body.contains("モード: URLエンコード"));
        assert!(body.contains("内容: hello"));
    }

    /// `notify_mode` のみ ON の場合はモード行だけ含むこと
    #[test]
    fn format_body_mode_only() {
        let settings = NotificationSettings {
            notify_result: false,
            ..enabled_settings()
        };
        let body = format_process_notification_body(&settings, RefineMode::Trim, "secret")
            .expect("通知本文が生成される");

        assert!(body.contains("モード:"));
        assert!(!body.contains("内容:"));
    }

    /// `notify_result` のみ ON の場合は内容行だけ含むこと
    #[test]
    fn format_body_result_only() {
        let settings = NotificationSettings {
            notify_mode: false,
            ..enabled_settings()
        };
        let body = format_process_notification_body(&settings, RefineMode::Trim, "hello")
            .expect("通知本文が生成される");

        assert!(!body.contains("モード:"));
        assert!(body.contains("内容: hello"));
    }

    /// 50 文字超のマルチバイト文字列を切り詰めること
    #[test]
    fn truncate_snippet_multibyte_over_limit() {
        let input = "あ".repeat(51);
        let snippet = truncate_notification_snippet(&input);
        assert!(snippet.ends_with("..."));
        assert_eq!(snippet.chars().count(), 50);
    }

    /// 50 文字以下はそのまま返すこと
    #[test]
    fn truncate_snippet_within_limit() {
        let input = "あ".repeat(50);
        assert_eq!(truncate_notification_snippet(&input), input);
    }
}

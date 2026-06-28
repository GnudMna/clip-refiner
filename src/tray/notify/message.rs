use std::sync::Arc;

use super::super::state::AppState;
use crate::config::NotificationSettings;
use crate::platform;
use crate::refiner::RefineMode;
use crate::security::format_public_snippet;

// ======================================================================
// 加工完了通知
// ======================================================================
/// 通知本文に含める加工結果スニペットの最大文字数
const NOTIFICATION_SNIPPET_MAX_CHARS: usize = 50;

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
            format_public_snippet(processed, NOTIFICATION_SNIPPET_MAX_CHARS)
        ));
    }

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
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
    platform::show_notification("変換完了", &body);
}

/// 画像加工完了時のデスクトップ通知を表示する
pub fn show_image_process_notification(
    state: &Arc<AppState>,
    mode: RefineMode,
    width: u32,
    height: u32,
) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if !settings.enabled {
        return;
    }

    let mut lines = Vec::new();
    if settings.notify_mode {
        lines.push(format!("モード: {}", mode.label()));
    }
    if settings.notify_result {
        lines.push(format!("内容: 画像 ({width}x{height})"));
    }

    if lines.is_empty() {
        return;
    }

    platform::show_notification("変換完了", &lines.join("\n"));
}

// ======================================================================
// 一時停止通知
// ======================================================================
/// 一時停止通知を表示すべきか判定する
pub(crate) fn should_show_pause_notification(settings: &NotificationSettings) -> bool {
    settings.enabled && settings.notify_pause
}

/// 一時停止状態に応じた通知本文を返す
pub(crate) fn pause_notification_body(paused: bool) -> &'static str {
    if paused {
        "クリップボード監視を一時停止しました"
    } else {
        "クリップボード監視を再開しました"
    }
}

/// 監視の一時停止または再開時のデスクトップ通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `paused` - 新しい一時停止状態 (`true`: 一時停止中, `false`: 監視中)
/// * `source` - 通知のタイトル(操作元を示す文字列、例: "ショートカット", "設定変更")
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if should_show_pause_notification(&settings) {
        platform::show_notification(source, pause_notification_body(paused));
    }
}

/// 成功通知が有効な場合のみデスクトップ通知を表示する
pub fn show_when_enabled(state: &Arc<AppState>, summary: &str, body: &str) {
    if state.with_config(|c| c.notification_settings.enabled) {
        platform::show_notification(summary, body);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotificationSettings;
    use crate::consts::SENSITIVE_SNIPPET_LABEL;

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
        let snippet = format_public_snippet(&input, NOTIFICATION_SNIPPET_MAX_CHARS);
        assert!(snippet.ends_with("..."));
        assert_eq!(snippet.chars().count(), 50);
    }

    /// 50 文字以下はそのまま返すこと
    #[test]
    fn truncate_snippet_within_limit() {
        let input = "あ".repeat(50);
        assert_eq!(
            format_public_snippet(&input, NOTIFICATION_SNIPPET_MAX_CHARS),
            input
        );
    }

    /// 機密らしい内容はマスクすること
    #[test]
    fn format_body_masks_sensitive_result() {
        let body = format_process_notification_body(
            &enabled_settings(),
            RefineMode::Trim,
            "api_key=supersecretvalue",
        )
        .expect("通知本文が生成される");

        assert!(body.contains(SENSITIVE_SNIPPET_LABEL));
        assert!(!body.contains("supersecretvalue"));
    }

    /// 通知無効時は一時停止通知を表示しないこと
    #[test]
    fn pause_notification_disabled_when_settings_off() {
        let settings = NotificationSettings::default();
        assert!(!should_show_pause_notification(&settings));
    }

    /// `notify_pause` のみ OFF の場合は一時停止通知を表示しないこと
    #[test]
    fn pause_notification_disabled_when_notify_pause_off() {
        let settings = NotificationSettings {
            notify_pause: false,
            ..enabled_settings()
        };
        assert!(!should_show_pause_notification(&settings));
    }

    /// 一時停止時の通知本文
    #[test]
    fn pause_notification_body_when_paused() {
        assert_eq!(
            pause_notification_body(true),
            "クリップボード監視を一時停止しました"
        );
    }

    /// 再開時の通知本文
    #[test]
    fn pause_notification_body_when_resumed() {
        assert_eq!(
            pause_notification_body(false),
            "クリップボード監視を再開しました"
        );
    }
}

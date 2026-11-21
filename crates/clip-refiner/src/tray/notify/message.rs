use std::sync::Arc;

use super::super::state::AppState;

use crate::config::NotificationSettings;
use crate::platform;
use crate::refiner::RefineMode;
use crate::security::format_public_snippet;

// ======================================================================
// 通知カテゴリ
// ======================================================================
/// 個別に ON/OFF できる通知の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCategory {
    /// 加工完了
    Process,
    /// 一時停止切替
    Pause,
    /// 履歴復元
    HistoryRestore,
    /// OCR 完了
    Ocr,
    /// 登録クリップ操作
    RegisteredClip,
    /// お気に入り操作
    Favorites,
    /// 連続記録操作
    ContinuousRecord,
}

/// 指定カテゴリの通知を表示すべきか判定する
pub(crate) fn category_enabled(
    settings: &NotificationSettings,
    category: NotificationCategory,
) -> bool {
    if !settings.enabled {
        return false;
    }

    match category {
        NotificationCategory::Process => settings.notify_process,
        NotificationCategory::Pause => settings.notify_pause,
        NotificationCategory::HistoryRestore => settings.notify_history_restore,
        NotificationCategory::Ocr => settings.notify_ocr,
        NotificationCategory::RegisteredClip => settings.notify_registered_clip,
        NotificationCategory::Favorites => settings.notify_favorites,
        NotificationCategory::ContinuousRecord => settings.notify_continuous_record,
    }
}

/// カテゴリ設定が有効な場合のみデスクトップ通知を表示する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態 (通知設定の参照用)
/// * `category` - 通知カテゴリ
/// * `summary` - 通知タイトル
/// * `body` - 通知本文
pub fn show_category_notification(
    state: &Arc<AppState>,
    category: NotificationCategory,
    summary: &str,
    body: &str,
) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if category_enabled(&settings, category) {
        platform::show_notification(summary, body);
    }
}

// ======================================================================
// 加工完了通知
// ======================================================================
/// 通知本文に含める加工結果スニペットの最大文字数
const NOTIFICATION_SNIPPET_MAX_CHARS: usize = 50;

/// 加工完了通知の本文を組み立てる
///
/// 通知が無効な場合は `None` を返す
///
/// # Arguments
/// * `settings` - 通知設定
/// * `pipeline` - 実行された変換モード列
/// * `processed` - 加工後のテキスト
///
/// # Returns
/// * `Option<String>` - 通知本文。表示不要な場合は `None`
pub(crate) fn format_process_notification_body(
    settings: &NotificationSettings,
    pipeline: &[RefineMode],
    processed: &str,
) -> Option<String> {
    if !category_enabled(settings, NotificationCategory::Process) {
        return None;
    }

    let mut lines = vec![format!("モード: {}", RefineMode::pipeline_label(pipeline))];
    if settings.notify_result {
        lines.push(format!(
            "内容: {}",
            format_public_snippet(processed, NOTIFICATION_SNIPPET_MAX_CHARS)
        ));
    }

    Some(lines.join("\n"))
}

/// 加工完了時のデスクトップ通知を表示する
///
/// 設定に応じて、実行されたモード名や加工後のテキストスニペットを通知内容に含める
///
/// # Arguments
/// * `state` - アプリケーションの共有状態 (通知設定の参照用)
/// * `pipeline` - 実行された変換モード列
/// * `processed` - 加工後のテキスト
pub fn show_process_notification(state: &Arc<AppState>, pipeline: &[RefineMode], processed: &str) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    let Some(body) = format_process_notification_body(&settings, pipeline, processed) else {
        return;
    };
    platform::show_notification("変換完了", &body);
}

/// 画像加工完了時のデスクトップ通知を表示する
///
/// 設定に応じて、実行されたモード名や画像サイズを通知内容に含める
///
/// # Arguments
/// * `state` - アプリケーションの共有状態 (通知設定の参照用)
/// * `mode` - 実行された変換モード
/// * `width` - 加工後画像の幅 (ピクセル)
/// * `height` - 加工後画像の高さ (ピクセル)
pub fn show_image_process_notification(
    state: &Arc<AppState>,
    mode: RefineMode,
    width: u32,
    height: u32,
) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if !category_enabled(&settings, NotificationCategory::Process) {
        return;
    }

    let mut lines = vec![format!("モード: {}", mode.label())];
    if settings.notify_result {
        lines.push(format!("内容: 画像 ({width}x{height})"));
    }

    platform::show_notification("変換完了", &lines.join("\n"));
}

// ======================================================================
// 一時停止通知
// ======================================================================
/// 一時停止通知を表示すべきか判定する
pub(crate) fn should_show_pause_notification(settings: &NotificationSettings) -> bool {
    category_enabled(settings, NotificationCategory::Pause)
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
/// * `source` - 通知のタイトル (操作元を示す文字列、例: "ショートカット", "設定変更")
pub fn show_pause_notification(state: &Arc<AppState>, paused: bool, source: &str) {
    let settings = state.with_config(|c| c.notification_settings.clone());
    if should_show_pause_notification(&settings) {
        platform::show_notification(source, pause_notification_body(paused));
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
        let mut settings = NotificationSettings::default();
        settings.enabled = true;
        settings.notify_result = true;
        settings
    }

    /// パイプライン時はモード名を連結して表示すること
    #[test]
    fn format_body_includes_pipeline_labels() {
        let body = format_process_notification_body(
            &enabled_settings(),
            &[RefineMode::UrlDecode, RefineMode::Trim],
            "hello",
        )
        .expect("通知本文が生成される");

        assert!(body.contains("モード: URLデコード → 全体をトリム"));
    }

    /// 通知無効時は本文を生成しないであるようにすること
    #[test]
    fn format_body_returns_none_when_disabled() {
        let settings = NotificationSettings::default();
        assert!(format_process_notification_body(&settings, &[RefineMode::Trim], "abc").is_none());
    }

    /// 加工完了 OFF 時は本文を生成しないであるようにすること
    #[test]
    fn format_body_returns_none_when_process_disabled() {
        let mut settings = enabled_settings();
        settings.notify_process = false;
        assert!(format_process_notification_body(&settings, &[RefineMode::Trim], "abc").is_none());
    }

    /// モード名と加工結果の両方を含むとすること
    #[test]
    fn format_body_includes_mode_and_result() {
        let body = format_process_notification_body(
            &enabled_settings(),
            &[RefineMode::UrlEncode],
            "hello",
        )
        .expect("通知本文が生成される");

        assert!(body.contains("モード: URLエンコード"));
        assert!(body.contains("内容: hello"));
    }

    /// `notify_result` OFF の場合はモード行だけ含むとすること
    #[test]
    fn format_body_mode_only_when_result_off() {
        let mut settings = enabled_settings();
        settings.notify_result = false;
        let body = format_process_notification_body(&settings, &[RefineMode::Trim], "secret")
            .expect("通知本文が生成される");

        assert!(body.contains("モード:"));
        assert!(!body.contains("内容:"));
    }

    /// 50 文字超のマルチバイト文字列を切り詰めるようにすること
    #[test]
    fn truncate_snippet_multibyte_over_limit() {
        let input = "あ".repeat(51);
        let snippet = format_public_snippet(&input, NOTIFICATION_SNIPPET_MAX_CHARS);
        assert!(snippet.ends_with("..."));
        assert_eq!(snippet.chars().count(), 50);
    }

    /// 50 文字以下はそのまま返すようにすること
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
            &[RefineMode::Trim],
            "api_key=supersecretvalue",
        )
        .expect("通知本文が生成される");

        assert!(body.contains(SENSITIVE_SNIPPET_LABEL));
        assert!(!body.contains("supersecretvalue"));
    }

    /// 通知無効時は一時停止通知を表示しないであるようにすること
    #[test]
    fn pause_notification_disabled_when_settings_off() {
        let settings = NotificationSettings::default();
        assert!(!should_show_pause_notification(&settings));
    }

    /// `notify_pause` のみ OFF の場合は一時停止通知を表示しないであるようにすること
    #[test]
    fn pause_notification_disabled_when_notify_pause_off() {
        let mut settings = enabled_settings();
        settings.notify_pause = false;
        assert!(!should_show_pause_notification(&settings));
    }

    /// 履歴復元カテゴリ OFF 時は通知しないであるようにすること
    #[test]
    fn history_restore_category_disabled() {
        let mut settings = enabled_settings();
        settings.notify_history_restore = false;
        assert!(!category_enabled(
            &settings,
            NotificationCategory::HistoryRestore
        ));
    }

    /// 一時停止時の通知本文できるようにすること
    #[test]
    fn pause_notification_body_when_paused() {
        assert_eq!(
            pause_notification_body(true),
            "クリップボード監視を一時停止しました"
        );
    }

    /// 再開時の通知本文できるようにすること
    #[test]
    fn pause_notification_body_when_resumed() {
        assert_eq!(
            pause_notification_body(false),
            "クリップボード監視を再開しました"
        );
    }
}

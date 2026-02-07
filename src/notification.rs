use crate::refiner::RefineMode;
use notify_rust::Notification;
use std::time::Duration;

/// 内部共通の通知表示関数
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `body` - 通知の本文。
fn show_notification(summary: &str, body: &str) {
    let _ = Notification::new()
        .summary(&format!("ClipRefiner - {}", summary))
        .body(body)
        .timeout(Duration::from_secs(3))
        .show();
}

/// 簡易的な通知を表示する
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `body` - 通知の本文。
pub fn show_simple_notification(summary: &str, body: &str) {
    show_notification(summary, body);
}

/// 処理完了通知を表示する
///
/// # Arguments
/// * `mode` - 実行された `RefineMode`。
/// * `text` - 加工後のテキスト。
pub fn show_process_notification(mode: RefineMode, text: &str) {
    let snippet = if text.chars().count() > 50 {
        format!("{}...", text.chars().take(47).collect::<String>())
    } else {
        text.to_string()
    };
    let body = format!("モード: {}\n内容: {}", mode.label(), snippet);
    show_notification("変換完了", &body);
}

/// anyhow::Error からエラー通知を表示する
///
/// # Arguments
/// * `summary` - 通知のタイトル。
/// * `err` - 表示する `anyhow::Error` インスタンス。
pub fn show_anyhow_error(summary: &str, err: &anyhow::Error) {
    let body = format!("{:#}", err); // {:#} で原因のチェーンを含めて表示
    show_simple_notification(summary, &body);
}

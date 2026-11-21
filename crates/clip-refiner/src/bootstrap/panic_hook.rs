//! パニックをログファイルへ記録するフック
//!
//! GUI サブシステムでは標準エラーの出力先が無く、既定フックの出力が失われる。
//! 非同期ログキューはプロセスの abort で失われうるため、ログファイルへ同期で書き込む

use std::any::Any;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::fmt::Write;
use std::panic;
use std::sync::Arc;
use std::thread;

use crate::logger::LocalDailyFileAppender;

// ======================================================================
// フック登録
// ======================================================================
/// パニック発生時にログファイルへ記録するフックを登録する
///
/// 記録後は既定フックへ委譲し、コンソールがある場合の標準エラー出力は従来どおり残す
///
/// # Arguments
/// * `appender` - 記録先のログファイル Appender
pub(super) fn install_panic_hook(appender: Arc<LocalDailyFileAppender>) {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let record = format_panic_record(
            &chrono::Utc::now(),
            thread::current().name(),
            &panic_message(info.payload()),
            info.location().map(ToString::to_string).as_deref(),
            &Backtrace::capture(),
        );
        // ログへ書けなくても既定フックの出力は残す
        let _ = appender.write_immediately(&record);
        default_hook(info);
    }));
}

// ======================================================================
// 記録の組み立て
// ======================================================================
/// パニックのペイロードからメッセージを取り出す
///
/// # Arguments
/// * `payload` - `panic!` に渡された値
///
/// # Returns
/// * `String` - `&str` / `String` の場合はその内容、それ以外は固定の説明文
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "(文字列以外のペイロード)".to_string()
    }
}

/// ログファイルへ書き込むパニック記録を組み立てる
///
/// 書式は `tracing_subscriber` の既定 fmt 出力 (UTC の RFC 3339 時刻とレベル) に合わせる
///
/// # Arguments
/// * `now` - 記録時刻
/// * `thread_name` - パニックしたスレッド名 (無名スレッドは `None`)
/// * `message` - パニックメッセージ
/// * `location` - 発生位置 (`file:line:col`)
/// * `backtrace` - バックトレース (取得していない場合は出力しない)
///
/// # Returns
/// * `String` - 末尾に改行を含む記録
fn format_panic_record(
    now: &chrono::DateTime<chrono::Utc>,
    thread_name: Option<&str>,
    message: &str,
    location: Option<&str>,
    backtrace: &Backtrace,
) -> String {
    let mut record = format!(
        "{} ERROR panic: スレッド '{}' がパニックしました: {} (位置: {})\n",
        now.format("%Y-%m-%dT%H:%M:%S%.6fZ"),
        thread_name.unwrap_or("<unnamed>"),
        message,
        location.unwrap_or("不明"),
    );
    if backtrace.status() == BacktraceStatus::Captured {
        let _ = writeln!(record, "バックトレース:\n{backtrace}");
    }
    record
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeZone;

    /// `&str` のペイロードからメッセージを取り出すこと
    #[test]
    fn panic_message_reads_str_payload() {
        let payload: Box<dyn Any + Send> = Box::new("boom");
        assert_eq!(panic_message(payload.as_ref()), "boom");
    }

    /// `String` のペイロードからメッセージを取り出すこと
    #[test]
    fn panic_message_reads_string_payload() {
        let payload: Box<dyn Any + Send> = Box::new(String::from("formatted 42"));
        assert_eq!(panic_message(payload.as_ref()), "formatted 42");
    }

    /// 文字列以外のペイロードは固定の説明文にすること
    #[test]
    fn panic_message_describes_non_string_payload() {
        let payload: Box<dyn Any + Send> = Box::new(42_u32);
        assert_eq!(panic_message(payload.as_ref()), "(文字列以外のペイロード)");
    }

    /// 時刻・スレッド名・メッセージ・位置を 1 行に含めること
    #[test]
    fn format_panic_record_contains_all_fields() {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 10, 3, 1, 2, 3)
            .single()
            .expect("有効な日時");
        let record = format_panic_record(
            &now,
            Some("clipboard-worker"),
            "boom",
            Some("src/lib.rs:10:5"),
            &Backtrace::disabled(),
        );
        assert_eq!(
            record,
            "2026-10-03T01:02:03.000000Z ERROR panic: スレッド 'clipboard-worker' がパニックしました: boom (位置: src/lib.rs:10:5)\n"
        );
    }

    /// スレッド名と位置が無い場合は代替表記にすること
    #[test]
    fn format_panic_record_uses_placeholders_when_missing() {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 10, 3, 1, 2, 3)
            .single()
            .expect("有効な日時");
        let record = format_panic_record(&now, None, "boom", None, &Backtrace::disabled());
        // 欠けた項目は空欄にせず代替表記で埋める
        assert!(record.contains("スレッド '<unnamed>'"), "{record}");
        assert!(record.contains("(位置: 不明)"), "{record}");
    }
}

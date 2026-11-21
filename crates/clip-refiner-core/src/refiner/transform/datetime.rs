use std::borrow::Cow;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

// ======================================================================
// 変換ヘルパー
// ======================================================================
/// ミリ秒として扱うタイムスタンプの絶対値の下限
///
/// 秒として解釈すると西暦 5138 年以降になる値で、ミリ秒なら 1973 年以降を表す
const MILLIS_THRESHOLD: u64 = 100_000_000_000;

/// Unixタイムスタンプをローカル日時文字列へ変換する
///
/// 絶対値が `MILLIS_THRESHOLD` 以上の場合はミリ秒として扱い、秒未満は切り捨てる
///
/// # Arguments
/// * `timestamp` - Unixタイムスタンプ (秒またはミリ秒)
///
/// # Returns
/// * `Option<String>` - 変換成功時はフォーマット済み文字列、失敗時は `None`
fn format_timestamp(timestamp: i64) -> Option<String> {
    let utc_dt = if timestamp.unsigned_abs() >= MILLIS_THRESHOLD {
        DateTime::from_timestamp_millis(timestamp)?
    } else {
        DateTime::from_timestamp(timestamp, 0)?
    };
    Some(
        utc_dt
            .with_timezone(&Local)
            .format(DATETIME_FORMAT)
            .to_string(),
    )
}

/// ローカル日時文字列を Unixタイムスタンプへ変換する
///
/// # Arguments
/// * `input` - `DATETIME_FORMAT` 形式の日時文字列
///
/// # Returns
/// * `Option<i64>` - 変換成功時は Unixタイムスタンプ (秒)、失敗時は `None`
fn parse_datetime(input: &str) -> Option<i64> {
    let naive_dt = NaiveDateTime::parse_from_str(input.trim(), DATETIME_FORMAT).ok()?;
    Some(Local.from_local_datetime(&naive_dt).single()?.timestamp())
}

// ======================================================================
// タイムスタンプ → 日時変換
// ======================================================================
/// Unixタイムスタンプをローカル日時文字列に変換する
///
/// # Arguments
/// * `input` - Unixタイムスタンプを表す文字列
///
/// # Returns
/// * `Cow<'_, str>` - "YYYY-MM-DD HH:MM:SS" 形式の日時文字列。変換失敗時は元の文字列を返す
pub fn timestamp_to_datetime_string(input: &str) -> Cow<'_, str> {
    if let Ok(timestamp) = input.trim().parse::<i64>()
        && let Some(formatted) = format_timestamp(timestamp)
    {
        return Cow::Owned(formatted);
    }
    Cow::Borrowed(input)
}

// ======================================================================
// 日時 → タイムスタンプ変換
// ======================================================================
/// ローカル日時文字列を Unixタイムスタンプに変換する
///
/// # Arguments
/// * `input` - "YYYY-MM-DD HH:MM:SS" 形式の日時文字列
///
/// # Returns
/// * `Cow<'_, str>` - Unixタイムスタンプを表す文字列。変換失敗時は元の文字列を返す
pub fn datetime_string_to_timestamp(input: &str) -> Cow<'_, str> {
    if let Some(timestamp) = parse_datetime(input) {
        return Cow::Owned(timestamp.to_string());
    }
    Cow::Borrowed(input)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な Unix タイムスタンプをローカル日時文字列に変換できるようにすること
    #[test]
    fn test_timestamp_to_datetime_string_success() {
        let timestamp = 1_672_531_200;
        let expected = format_timestamp(timestamp).expect("タイムスタンプ変換に失敗");
        assert_eq!(timestamp_to_datetime_string("1672531200"), expected);
    }

    /// ミリ秒 (13 桁) のタイムスタンプは秒に換算して変換すること
    #[test]
    fn test_timestamp_to_datetime_string_millis() {
        let expected = format_timestamp(1_700_000_000).expect("タイムスタンプ変換に失敗");
        assert_eq!(timestamp_to_datetime_string("1700000000000"), expected);
        // 秒未満は切り捨てる
        assert_eq!(timestamp_to_datetime_string("1700000000999"), expected);
        // 負のミリ秒も秒に換算する
        assert_eq!(
            timestamp_to_datetime_string("-1700000000000"),
            format_timestamp(-1_700_000_000).expect("タイムスタンプ変換に失敗")
        );
        // 秒の最大桁 (10 桁) は秒のまま扱う
        assert_eq!(
            timestamp_to_datetime_string("9999999999"),
            format_timestamp(9_999_999_999).expect("タイムスタンプ変換に失敗")
        );
    }

    /// 不正なタイムスタンプは元の文字列を返すようにすること
    #[test]
    fn test_timestamp_to_datetime_string_invalid() {
        assert_eq!(
            timestamp_to_datetime_string("invalid_timestamp"),
            "invalid_timestamp"
        );
    }

    /// ローカル日時文字列を Unix タイムスタンプに変換できるようにすること
    #[test]
    fn test_datetime_string_to_timestamp_success() {
        let sample = format_timestamp(1_672_531_200).expect("タイムスタンプ変換に失敗");
        let expected = parse_datetime(&sample)
            .expect("日時パースに失敗")
            .to_string();

        assert_eq!(datetime_string_to_timestamp(&sample), expected);
    }

    /// 不正な日時文字列は元の文字列を返すようにすること
    #[test]
    fn test_datetime_string_to_timestamp_invalid() {
        assert_eq!(
            datetime_string_to_timestamp("not-a-datetime"),
            "not-a-datetime"
        );
    }

    /// ローカルタイムゾーンでタイムスタンプと日時の往復変換が一致すること
    #[test]
    fn test_local_roundtrip() {
        let timestamp = 1_700_000_000_i64;
        let formatted = format_timestamp(timestamp).expect("タイムスタンプ変換に失敗");
        let parsed = parse_datetime(&formatted).expect("日時パースに失敗");
        assert_eq!(parsed, timestamp);
    }
}

use std::borrow::Cow;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

// ======================================================================
// タイムスタンプ → 日時変換
// ======================================================================
/// Unixタイムスタンプを日時文字列に変換する
///
/// # Arguments
/// * `input` - Unixタイムスタンプを表す文字列
///
/// # Returns
/// * `Cow<'_, str>` - "YYYY-MM-DD HH:MM:SS"形式の日時文字列。変換失敗時は元の文字列を返す。
pub fn timestamp_to_datetime_string(input: &str) -> Cow<'_, str> {
    if let Ok(timestamp) = input.trim().parse::<i64>()
        && let Some(utc_dt) = DateTime::from_timestamp(timestamp, 0)
    {
        let local_dt: DateTime<Local> = utc_dt.with_timezone(&Local);
        return Cow::Owned(local_dt.format(DATETIME_FORMAT).to_string());
    }
    Cow::Borrowed(input)
}

// ======================================================================
// 日時 → タイムスタンプ変換
// ======================================================================
/// 日時文字列をUnixタイムスタンプに変換する
///
/// # Arguments
/// * `input` - "YYYY-MM-DD HH:MM:SS"形式の日時文字列
///
/// # Returns
/// * `Cow<'_, str>` - Unixタイムスタンプを表す文字列。変換失敗時は元の文字列を返す。
pub fn datetime_string_to_timestamp(input: &str) -> Cow<'_, str> {
    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(input.trim(), DATETIME_FORMAT)
        && let Some(local_dt) = Local.from_local_datetime(&naive_dt).single()
    {
        return Cow::Owned(local_dt.timestamp().to_string());
    }
    Cow::Borrowed(input)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 有効な Unix タイムスタンプを日時文字列に変換できること
    #[test]
    fn test_timestamp_to_datetime_string_success() {
        // ローカルタイムゾーンでの期待値を算出
        let dt: DateTime<Local> = DateTime::from_timestamp(1672531200, 0)
            .unwrap()
            .with_timezone(&Local);
        let expected_str = dt.format(DATETIME_FORMAT).to_string();
        assert_eq!(timestamp_to_datetime_string("1672531200"), expected_str);
    }

    /// 不正なタイムスタンプは元の文字列を返すこと
    #[test]
    fn test_timestamp_to_datetime_string_invalid() {
        assert_eq!(
            timestamp_to_datetime_string("invalid_timestamp"),
            "invalid_timestamp"
        );
    }

    /// 日時文字列を Unix タイムスタンプに変換できること
    #[test]
    fn test_datetime_string_to_timestamp_success() {
        let naive_dt =
            NaiveDateTime::parse_from_str("2023-01-01 09:00:00", DATETIME_FORMAT).unwrap();
        let local_dt = Local.from_local_datetime(&naive_dt).single().unwrap();
        let expected_timestamp = local_dt.timestamp().to_string();

        assert_eq!(
            datetime_string_to_timestamp("2023-01-01 09:00:00"),
            expected_timestamp
        );
    }
}

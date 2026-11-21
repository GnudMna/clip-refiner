use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Unixタイムスタンプを日時文字列に変換する
///
/// # Arguments
/// * `input` - Unixタイムスタンプを表す文字列
///
/// # Returns
/// * `String` - "YYYY-MM-DD HH:MM:SS"形式の日時文字列。変換失敗時は元の文字列を返す。
pub fn timestamp_to_datetime_string(input: &str) -> String {
    if let Ok(timestamp) = input.trim().parse::<i64>()
        && let Some(utc_dt) = DateTime::from_timestamp(timestamp, 0)
    {
        let local_dt: DateTime<Local> = utc_dt.with_timezone(&Local);
        return local_dt.format(DATETIME_FORMAT).to_string();
    }
    input.to_string()
}

/// 日時文字列をUnixタイムスタンプに変換する
///
/// # Arguments
/// * `input` - "YYYY-MM-DD HH:MM:SS"形式の日時文字列
///
/// # Returns
/// * `String` - Unixタイムスタンプを表す文字列。変換失敗時は元の文字列を返す。
pub fn datetime_string_to_timestamp(input: &str) -> String {
    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(input.trim(), DATETIME_FORMAT)
        && let Some(local_dt) = Local.from_local_datetime(&naive_dt).single()
    {
        return local_dt.timestamp().to_string();
    }
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_datetime_string_success() {
        // `date -r 1672531200` is `2023-01-01 09:00:00 JST` (example local time)
        let dt: DateTime<Local> = DateTime::from_timestamp(1672531200, 0)
            .unwrap()
            .with_timezone(&Local);
        let expected_str = dt.format(DATETIME_FORMAT).to_string();
        assert_eq!(timestamp_to_datetime_string("1672531200"), expected_str);
    }

    #[test]
    fn test_timestamp_to_datetime_string_invalid() {
        assert_eq!(
            timestamp_to_datetime_string("invalid_timestamp"),
            "invalid_timestamp"
        );
        assert_eq!(
            timestamp_to_datetime_string("9999999999999999999"), // Too large for i64
            "9999999999999999999"
        );
    }

    #[test]
    fn test_datetime_string_to_timestamp_success() {
        // JSTでの`2023-01-01 09:00:00`のタイムスタンプ (例)
        let naive_dt =
            NaiveDateTime::parse_from_str("2023-01-01 09:00:00", DATETIME_FORMAT).unwrap();
        let local_dt = Local.from_local_datetime(&naive_dt).single().unwrap();
        let expected_timestamp = local_dt.timestamp().to_string();

        assert_eq!(
            datetime_string_to_timestamp("2023-01-01 09:00:00"),
            expected_timestamp
        );
    }

    #[test]
    fn test_datetime_string_to_timestamp_invalid() {
        assert_eq!(
            datetime_string_to_timestamp("invalid datetime"),
            "invalid datetime"
        );
        assert_eq!(
            datetime_string_to_timestamp("2023/01/01 09:00:00"),
            "2023/01/01 09:00:00"
        );
    }
}

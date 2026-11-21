use std::borrow::Cow;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

// ======================================================================
// タイムゾーン
// ======================================================================
/// 日時文字列の解釈・出力に使うタイムゾーン
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatetimeZone {
    /// OS のローカルタイムゾーン
    Local,
    /// UTC (テストでの検証および将来のモード拡張用)
    #[allow(dead_code)]
    Utc,
}

// ======================================================================
// 変換ヘルパー
// ======================================================================
/// Unixタイムスタンプを日時文字列へ変換する
///
/// # Arguments
/// * `timestamp` - Unixタイムスタンプ (秒)
/// * `zone` - 出力に使うタイムゾーン
///
/// # Returns
/// * `Option<String>` - 変換成功時はフォーマット済み文字列、失敗時は `None`
fn format_timestamp(timestamp: i64, zone: DatetimeZone) -> Option<String> {
    let utc_dt = DateTime::from_timestamp(timestamp, 0)?;
    let formatted = match zone {
        DatetimeZone::Local => utc_dt
            .with_timezone(&Local)
            .format(DATETIME_FORMAT)
            .to_string(),
        DatetimeZone::Utc => utc_dt.format(DATETIME_FORMAT).to_string(),
    };
    Some(formatted)
}

/// 日時文字列を Unixタイムスタンプへ変換する
///
/// # Arguments
/// * `input` - `DATETIME_FORMAT` 形式の日時文字列
/// * `zone` - 入力文字列の解釈に使うタイムゾーン
///
/// # Returns
/// * `Option<i64>` - 変換成功時は Unixタイムスタンプ (秒)、失敗時は `None`
fn parse_datetime(input: &str, zone: DatetimeZone) -> Option<i64> {
    let naive_dt = NaiveDateTime::parse_from_str(input.trim(), DATETIME_FORMAT).ok()?;
    Some(match zone {
        DatetimeZone::Local => Local.from_local_datetime(&naive_dt).single()?.timestamp(),
        DatetimeZone::Utc => Utc.from_utc_datetime(&naive_dt).timestamp(),
    })
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
        && let Some(formatted) = format_timestamp(timestamp, DatetimeZone::Local)
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
    if let Some(timestamp) = parse_datetime(input, DatetimeZone::Local) {
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

    /// 有効な Unix タイムスタンプをローカル日時文字列に変換できること
    #[test]
    fn test_timestamp_to_datetime_string_success() {
        let timestamp = 1_672_531_200;
        let expected =
            format_timestamp(timestamp, DatetimeZone::Local).expect("タイムスタンプ変換に失敗");
        assert_eq!(timestamp_to_datetime_string("1672531200"), expected);
    }

    /// 不正なタイムスタンプは元の文字列を返すこと
    #[test]
    fn test_timestamp_to_datetime_string_invalid() {
        assert_eq!(
            timestamp_to_datetime_string("invalid_timestamp"),
            "invalid_timestamp"
        );
    }

    /// ローカル日時文字列を Unix タイムスタンプに変換できること
    #[test]
    fn test_datetime_string_to_timestamp_success() {
        let sample =
            format_timestamp(1_672_531_200, DatetimeZone::Local).expect("タイムスタンプ変換に失敗");
        let expected = parse_datetime(&sample, DatetimeZone::Local)
            .expect("日時パースに失敗")
            .to_string();

        assert_eq!(datetime_string_to_timestamp(&sample), expected);
    }

    /// 不正な日時文字列は元の文字列を返すこと
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
        let formatted =
            format_timestamp(timestamp, DatetimeZone::Local).expect("タイムスタンプ変換に失敗");
        let parsed = parse_datetime(&formatted, DatetimeZone::Local).expect("日時パースに失敗");
        assert_eq!(parsed, timestamp);
    }

    /// UTC でもタイムスタンプと日時の往復変換が一致すること
    #[test]
    fn test_utc_roundtrip() {
        let timestamp = 1_700_000_000_i64;
        let formatted =
            format_timestamp(timestamp, DatetimeZone::Utc).expect("タイムスタンプ変換に失敗");
        let parsed = parse_datetime(&formatted, DatetimeZone::Utc).expect("日時パースに失敗");
        assert_eq!(parsed, timestamp);
    }

    /// Local と UTC で異なる文字列になること (タイムゾーン差の確認)
    #[test]
    fn test_local_and_utc_differ_when_offset_nonzero() {
        let timestamp = 0_i64;
        let local = format_timestamp(timestamp, DatetimeZone::Local).expect("ローカル変換に失敗");
        let utc = format_timestamp(timestamp, DatetimeZone::Utc).expect("UTC変換に失敗");
        if Local::now().offset().local_minus_utc() != 0 {
            assert_ne!(local, utc);
        } else {
            assert_eq!(local, utc);
        }
    }
}

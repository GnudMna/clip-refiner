use std::borrow::Cow;

use jiff::civil::{Date, DateTime, Time};
use jiff::fmt::{rfc2822, strtime};
use jiff::tz::{AmbiguousOffset, Offset, TimeZone};
use jiff::{Timestamp, ToSpan, ZonedDateTime};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const DATETIME_MILLIS_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";
const JAPANESE_FORMAT: &str = "%Y年%m月%d日 %H時%M分%S秒";
/// 和文形式として受け付ける書式 (秒の省略と `HH:MM:SS` の時刻を許す)
const JAPANESE_PARSE_FORMATS: [&str; 4] = [
    JAPANESE_FORMAT,
    "%Y年%m月%d日 %H時%M分",
    "%Y年%m月%d日 %H:%M:%S",
    "%Y年%m月%d日 %H:%M",
];
const JAPANESE_DATE_PARSE_FORMAT: &str = "%Y年%m月%d日";

/// 和暦の元号名と、元号の年に足して西暦にする値 (西暦 = 元号の年 + 値)
const ERA_NAMES: [(&str, i32); 5] = [
    ("令和", 2018),
    ("平成", 1988),
    ("昭和", 1925),
    ("大正", 1911),
    ("明治", 1867),
];
/// 和暦の略号 (`R5.11.15` など)、元号の年に足して西暦にする値、元号の期間 (開始日と終了日、ともに含む)
///
/// 略号は `M1.2.3` のようなバージョン番号と紛れやすいため、期間外の日付は和暦として扱わない
const ERA_INITIALS: [(char, i32, EraDate, EraDate); 5] = [
    ('R', 2018, (2019, 5, 1), (9999, 12, 31)),
    ('H', 1988, (1989, 1, 8), (2019, 4, 30)),
    ('S', 1925, (1926, 12, 25), (1989, 1, 7)),
    ('T', 1911, (1912, 7, 30), (1926, 12, 24)),
    ('M', 1867, (1868, 1, 25), (1912, 7, 29)),
];

/// 年・月・日の組
type EraDate = (i32, u32, u32);

/// 月名・米国式として受け付ける日付の書式
///
/// `%B` (英語の月名) は正式名、`%b` は略称だけを受け付けるため、月名の書式は両方を並べる
const WESTERN_DATE_FORMATS: [&str; 7] = [
    "%B %d, %Y",
    "%b %d, %Y",
    "%B %d %Y",
    "%b %d %Y",
    "%d %B %Y",
    "%d %b %Y",
    "%m/%d/%Y",
];
/// 月名・米国式の日付に続けて受け付ける時刻の書式 (12 時間制は `AM` / `PM` 付き)
const WESTERN_TIME_FORMATS: [&str; 4] = ["%H:%M:%S%.f", "%H:%M", "%I:%M:%S %p", "%I:%M %p"];

// ======================================================================
// 変換ヘルパー
// ======================================================================
/// ミリ秒として扱うタイムスタンプの絶対値の下限
///
/// 秒として解釈すると西暦 5138 年以降になる値で、ミリ秒なら 1973 年以降を表す
const MILLIS_THRESHOLD: u64 = 100_000_000_000;
/// マイクロ秒として扱うタイムスタンプの絶対値の下限 (ミリ秒なら西暦 5138 年以降になる値)
const MICROS_THRESHOLD: u64 = 100_000_000_000_000;
/// ナノ秒として扱うタイムスタンプの絶対値の下限 (マイクロ秒なら西暦 5138 年以降になる値)
const NANOS_THRESHOLD: u64 = 100_000_000_000_000_000;

/// Excel のシリアル値として扱う日数の下限
///
/// 1900 年 2 月 29 日 (Excel が実在しない日として数える) より後の 1900 年 3 月 1 日
const EXCEL_SERIAL_MIN: i64 = 61;
/// Excel のシリアル値として扱う日数の上限 (9999 年 12 月 31 日)
const EXCEL_SERIAL_MAX: i64 = 2_958_465;
/// Excel のシリアル値の 0 日目 (1900 年のうるう年の誤りを補正した基準日)
const EXCEL_EPOCH: (i32, u32, u32) = (1899, 12, 30);
const MILLIS_PER_DAY: u128 = 86_400_000;

/// 解析した日時
#[derive(Clone, Copy)]
struct Moment {
    utc: Timestamp,
    /// ミリ秒を保つかどうか (出力に小数点以下 3 桁を付け、タイムスタンプはミリ秒にする)
    has_millis: bool,
    /// 入力が Unixタイムスタンプの数値だったかどうか
    is_unix_number: bool,
}

impl Moment {
    /// Unixタイムスタンプ (`has_millis` の場合はミリ秒、それ以外は秒) を取得する
    fn timestamp(self) -> i64 {
        // 負の値は 0 方向への切り捨てではなく、過去方向へ切り下げる
        let nanos = self.utc.as_nanosecond();
        let divisor = if self.has_millis {
            1_000_000
        } else {
            1_000_000_000
        };
        i64::try_from(nanos.div_euclid(divisor)).unwrap_or_default()
    }
}

/// タイムスタンプから日時文字列へ変換するときの出力形式
#[derive(Clone, Copy)]
enum OutputStyle {
    /// `YYYY-MM-DD HH:MM:SS` (ローカルタイムゾーン)
    Local,
    /// `YYYY-MM-DD HH:MM:SS` (UTC)
    Utc,
    /// ISO 8601 / RFC 3339 (ローカルタイムゾーンのオフセット付き)
    Iso8601Local,
    /// ISO 8601 / RFC 3339 (UTC。`Z` 付き)
    Iso8601Utc,
    /// RFC 2822 (ローカルタイムゾーンのオフセット付き)
    Rfc2822,
    /// `YYYY年MM月DD日 HH時MM分SS秒` (ローカルタイムゾーン)
    Japanese,
}

/// 日時を指定の出力形式の文字列へ変換する
///
/// ミリ秒を持つ日時は `Local`・`Utc`・`Iso8601Local`・`Iso8601Utc` で小数点以下 3 桁として出力し、
/// 小数点以下を表せない `Rfc2822`・`Japanese` では切り捨てる
/// `Rfc2822` はローカルタイムゾーンの年が 0〜9999 の範囲外だと表せない
///
/// # Arguments
/// * `moment` - 変換する日時
/// * `style` - 出力形式
///
/// # Returns
/// * `Option<String>` - フォーマット済み文字列。表せない日時の場合は `None`
fn format_moment(moment: Moment, style: OutputStyle) -> Option<String> {
    let plain_format = if moment.has_millis {
        DATETIME_MILLIS_FORMAT
    } else {
        DATETIME_FORMAT
    };
    let local_dt = moment.utc.to_zoned_datetime(TimeZone::system());
    let utc_dt = moment.utc.to_zoned_datetime(TimeZone::UTC);
    match style {
        OutputStyle::Local => strtime::format(plain_format, &local_dt).ok(),
        OutputStyle::Utc => strtime::format(plain_format, &utc_dt).ok(),
        OutputStyle::Iso8601Local => format_rfc3339(&local_dt, moment.has_millis),
        OutputStyle::Iso8601Utc => format_rfc3339(&utc_dt, moment.has_millis),
        // 年が範囲外の日時は RFC 2822 で表せない
        OutputStyle::Rfc2822 => (0..=9999)
            .contains(&local_dt.year())
            .then(|| rfc2822::to_string(&local_dt).ok())?,
        OutputStyle::Japanese => strtime::format(JAPANESE_FORMAT, &local_dt).ok(),
    }
}

/// 日時を RFC 3339 形式で出力する
///
/// オフセットが 0 のときは `Z`、それ以外は `+09:00` の形で付ける
///
/// # Arguments
/// * `dt` - 出力する日時
/// * `has_millis` - 小数点以下 3 桁のミリ秒を付けるかどうか
///
/// # Returns
/// * `Option<String>` - フォーマット済み文字列。書式化できない場合は `None`
fn format_rfc3339(dt: &ZonedDateTime, has_millis: bool) -> Option<String> {
    let format = if has_millis {
        "%Y-%m-%dT%H:%M:%S%.3f"
    } else {
        "%Y-%m-%dT%H:%M:%S"
    };
    let mut text = strtime::format(format, dt).ok()?;
    if dt.offset().is_zero() {
        text.push('Z');
    } else {
        text.push_str(&strtime::format("%:z", dt).ok()?);
    }
    Some(text)
}

/// 複数行の日時を行ごとに指定の出力形式へ変換する
///
/// 入力の形式は行ごとに `parse_moment` で判定する
///
/// # Arguments
/// * `input` - 日時を表す文字列
/// * `style` - 出力形式
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変換できない行・出力形式で表せない日時の行は元のまま残す
fn convert_timestamps(input: &str, style: OutputStyle) -> Cow<'_, str> {
    super::utils::process_lines(input, |line| {
        parse_moment(line).and_then(|moment| format_moment(moment, style).map(Cow::Owned))
    })
}

/// 入力の形式を判定して日時へ変換する
///
/// 数値 (Unixタイムスタンプ・Excel のシリアル値)・ISO 8601 系・RFC 2822・和文形式・和暦・
/// 月名と米国式の順に試し、最初に解釈できた結果を返す
///
/// # Arguments
/// * `input` - 日時を表す文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、どの形式でもない場合は `None`
fn parse_moment(input: &str) -> Option<Moment> {
    let input = input.trim();
    parse_number(input)
        .or_else(|| parse_datetime(input))
        .or_else(|| parse_rfc2822(input))
        .or_else(|| parse_japanese(input))
        .or_else(|| parse_japanese_era(input))
        .or_else(|| parse_western(input))
}

/// ローカルタイムゾーンの日時を `Moment` へ変換する
///
/// # Arguments
/// * `naive` - タイムゾーンを持たない日時
/// * `has_millis` - ミリ秒を保つかどうか
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、夏時間の切り替えで存在しない時刻の場合は `None`
///   重複する時刻は早い方 (切り替え前) を採用する
fn local_moment(naive: DateTime, has_millis: bool) -> Option<Moment> {
    Some(Moment {
        utc: local_timestamp(naive)?,
        has_millis,
        is_unix_number: false,
    })
}

/// ローカルタイムゾーンの日時を `Timestamp` へ変換する
///
/// # Arguments
/// * `naive` - タイムゾーンを持たない日時
///
/// # Returns
/// * `Option<Timestamp>` - 変換成功時は日時、夏時間の切り替えで存在しない時刻の場合は `None`
///   重複する時刻は早い方 (切り替え前) を採用する
fn local_timestamp(naive: DateTime) -> Option<Timestamp> {
    let offset = match TimeZone::system().to_ambiguous_timestamp(naive).offset() {
        AmbiguousOffset::Unambiguous { offset } | AmbiguousOffset::Fold { before: offset, .. } => {
            offset
        }
        AmbiguousOffset::Gap { .. } => return None,
    };
    offset.to_timestamp(naive).ok()
}

/// Unixタイムスタンプの整数を日時へ変換する
///
/// 絶対値が `MILLIS_THRESHOLD` 以上はミリ秒、`MICROS_THRESHOLD` 以上はマイクロ秒、
/// `NANOS_THRESHOLD` 以上はナノ秒として扱う。ミリ秒より細かい桁は切り捨てる
///
/// # Arguments
/// * `timestamp` - Unixタイムスタンプ (秒・ミリ秒・マイクロ秒・ナノ秒)
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、範囲外の場合は `None`
fn moment_from_unix(timestamp: i64) -> Option<Moment> {
    let abs = timestamp.unsigned_abs();
    let (utc, has_millis) = if abs < MILLIS_THRESHOLD {
        (Timestamp::from_second(timestamp).ok()?, false)
    } else {
        let millis = if abs < MICROS_THRESHOLD {
            timestamp
        } else if abs < NANOS_THRESHOLD {
            timestamp.div_euclid(1_000)
        } else {
            timestamp.div_euclid(1_000_000)
        };
        (Timestamp::from_millisecond(millis).ok()?, true)
    };
    Some(Moment {
        utc,
        has_millis,
        is_unix_number: true,
    })
}

/// 数値の入力を Unixタイムスタンプまたは Excel のシリアル値として日時へ変換する
///
/// 整数部が `EXCEL_SERIAL_MIN` 以上 `EXCEL_SERIAL_MAX` 以下で小数部を持つ正の数は Excel のシリアル値
/// (小数部は時刻、ローカルタイムゾーン) として扱う。`2024` や `8080` のような整数だけの値は
/// シリアル値にせず、`moment_from_unix` の桁数判定に従う。それ以外の小数は
/// 秒の Unixタイムスタンプ (小数点以下 4 桁目以降は切り捨て) として扱う
///
/// # Arguments
/// * `input` - 前後の空白を除いた入力
///
/// # Returns
/// * `Option<Moment>` - 数値として解釈できた場合は日時、そうでなければ `None`
fn parse_number(input: &str) -> Option<Moment> {
    let unsigned = input.strip_prefix(['-', '+']).unwrap_or(input);
    let (int_part, frac_part) = match unsigned.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (unsigned, None),
    };
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !is_digits(int_part) || frac_part.is_some_and(|frac| !is_digits(frac)) {
        return None;
    }
    let negative = input.starts_with('-');
    let int_value: i64 = int_part.parse().ok()?;

    let Some(frac_part) = frac_part else {
        return moment_from_unix(input.parse().ok()?);
    };
    if !negative && (EXCEL_SERIAL_MIN..=EXCEL_SERIAL_MAX).contains(&int_value) {
        return parse_excel_serial(int_value, frac_part);
    }
    if int_value.unsigned_abs() >= MILLIS_THRESHOLD {
        return None;
    }
    // 小数点以下 3 桁をミリ秒として足す
    let frac_millis: i64 = format!("{frac_part:0<3}")[..3].parse().ok()?;
    let millis = int_value.checked_mul(1000)?.checked_add(frac_millis)?;
    let millis = if negative { -millis } else { millis };
    Some(Moment {
        utc: Timestamp::from_millisecond(millis).ok()?,
        has_millis: true,
        is_unix_number: true,
    })
}

/// Excel のシリアル値を日時へ変換する
///
/// 小数部は 1 日に対する割合として時刻にする (ミリ秒未満は四捨五入)
/// タイムゾーンを持たない値なので、ローカルタイムゾーンとして扱う
///
/// # Arguments
/// * `days` - シリアル値の整数部 (日数)
/// * `frac` - シリアル値の小数点以下の桁
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、失敗時は `None`
fn parse_excel_serial(days: i64, frac: &str) -> Option<Moment> {
    // 浮動小数点を使わず、小数点以下 12 桁までを整数演算で 1 日のミリ秒へ換算する
    let digits = &frac[..frac.len().min(12)];
    let numerator: u128 = digits.parse().ok()?;
    let denominator = 10_u128.pow(u32::try_from(digits.len()).ok()?);
    let rounded = (numerator * MILLIS_PER_DAY + denominator / 2) / denominator;
    let day_millis = i64::try_from(rounded).ok()?;
    let (year, month, day) = EXCEL_EPOCH;
    let naive = Date::new(
        i16::try_from(year).ok()?,
        i8::try_from(month).ok()?,
        i8::try_from(day).ok()?,
    )
    .ok()?
    .at(0, 0, 0, 0)
    .checked_add(days.days())
    .ok()?
    .checked_add(day_millis.milliseconds())
    .ok()?;
    local_moment(naive, day_millis % 1000 != 0)
}

/// 日時文字列を日時へ変換する
///
/// 受け付ける形式は次のとおり
/// - 日付: `YYYY-MM-DD` または `YYYY/MM/DD` (月日は 1 桁でもよい)
/// - 日付と時刻の区切り: スペースまたは `T`
/// - 時刻: `HH:MM:SS` または `HH:MM`。秒に小数点以下を付けてもよい
/// - タイムゾーン: `Z`・`+09:00`・`+0900`・`+09`。省略時はローカルタイムゾーン
///
/// 時刻を省略した日付だけの入力は、ローカルタイムゾーンの 0 時として扱う
/// 秒に小数点以下がある場合はミリ秒 (小数点以下 4 桁目以降は切り捨て) を保つ
///
/// # Arguments
/// * `input` - 日時文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、失敗時は `None`
fn parse_datetime(input: &str) -> Option<Moment> {
    let input = input.trim();
    let (date_str, time_part) = match input.find([' ', 'T', 't']) {
        Some(pos) => (&input[..pos], Some(input[pos + 1..].trim_start())),
        None => (input, None),
    };
    let date = parse_date(date_str)?;

    let (time, offset, has_fraction) = match time_part {
        Some(time_part) => {
            // 時刻の文字にはオフセットの先頭 (`Z`・`+`・`-`) が現れない
            let (time_str, offset) = match time_part.find(['Z', 'z', '+', '-']) {
                Some(pos) => (
                    time_part[..pos].trim_end(),
                    Some(parse_offset(&time_part[pos..])?),
                ),
                None => (time_part, None),
            };
            let time = Time::strptime("%H:%M:%S%.f", time_str)
                .or_else(|_| Time::strptime("%H:%M", time_str))
                .ok()?;
            (time, offset, time_str.contains('.'))
        }
        None => (Time::midnight(), None, false),
    };

    let naive = date.to_datetime(time);
    let utc = match offset {
        Some(offset) => offset.to_timestamp(naive).ok()?,
        None => local_timestamp(naive)?,
    };
    Some(Moment {
        utc,
        has_millis: has_fraction,
        is_unix_number: false,
    })
}

/// RFC 2822 形式 (`Wed, 15 Nov 2023 07:13:20 +0900`) の日時文字列を日時へ変換する
///
/// # Arguments
/// * `input` - 日時文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、失敗時は `None`
fn parse_rfc2822(input: &str) -> Option<Moment> {
    let datetime = rfc2822::parse(input.trim()).ok()?;
    Some(Moment {
        utc: datetime.timestamp(),
        has_millis: false,
        is_unix_number: false,
    })
}

/// 和文形式 (`2023年11月15日 07時13分20秒`) の日時文字列を日時へ変換する
///
/// 秒や時刻を省略した入力も受け付け、日付だけの入力は 0 時として扱う
/// 時刻は `07:13:20` の形でもよい。タイムゾーンはローカルタイムゾーンとして扱う
///
/// # Arguments
/// * `input` - 日時文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、失敗時は `None`
fn parse_japanese(input: &str) -> Option<Moment> {
    let input = input.trim();
    let naive = JAPANESE_PARSE_FORMATS
        .iter()
        .find_map(|format| DateTime::strptime(format, input).ok())
        .or_else(|| {
            Date::strptime(JAPANESE_DATE_PARSE_FORMAT, input)
                .ok()
                .map(|date| date.at(0, 0, 0, 0))
        })?;
    local_moment(naive, false)
}

/// 和暦の日時文字列を日時へ変換する
///
/// 元号は `令和`・`平成`・`昭和`・`大正`・`明治` と略号 `R`・`H`・`S`・`T`・`M` に対応し、
/// 元年は `元` と書ける。`令和5年11月15日 07時13分` のような和文形式と、
/// `R5.11.15 07:13` のような区切り文字 (`.`・`/`・`-`) の形式を受け付ける
/// 略号形式はバージョン番号などとの誤検知を避けるため、月日を 2 桁に限り、元号の期間内の日付だけを受け付ける
/// 元号名の形式は改元日による範囲の検証をしない
///
/// # Arguments
/// * `input` - 和暦の日時文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、和暦でない場合や失敗時は `None`
fn parse_japanese_era(input: &str) -> Option<Moment> {
    // 略号形式のときだけ元号の期間を持つ
    let (offset, rest, span) = ERA_NAMES
        .iter()
        .find_map(|(name, offset)| input.strip_prefix(name).map(|rest| (*offset, rest, None)))
        .or_else(|| {
            let mut chars = input.chars();
            let initial = chars.next()?.to_ascii_uppercase();
            ERA_INITIALS
                .iter()
                .find(|(era_initial, ..)| *era_initial == initial)
                .map(|(_, offset, start, end)| (*offset, chars.as_str(), Some((*start, *end))))
        })?;

    let (era_year, rest) = if let Some(rest) = rest.strip_prefix('元') {
        (1, rest)
    } else {
        let digits_len = rest.bytes().take(2).take_while(u8::is_ascii_digit).count();
        if digits_len == 0 {
            return None;
        }
        (rest[..digits_len].parse::<i32>().ok()?, &rest[digits_len..])
    };
    if era_year == 0 {
        return None;
    }
    let year = offset + era_year;

    let Some((start, end)) = span else {
        let rest = rest.strip_prefix('年')?;
        return parse_japanese(&format!("{year:04}年{rest}"));
    };
    let rest = rest.strip_prefix(['.', '/', '-'])?;
    let (date_part, time_part) = match rest.find([' ', 'T', 't']) {
        Some(pos) => rest.split_at(pos),
        None => (rest, ""),
    };
    let mut parts = date_part.split(['.', '/', '-']);
    let (month, day) = (parts.next()?, parts.next()?);
    if parts.next().is_some() || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let date = (year, month.parse::<u32>().ok()?, day.parse::<u32>().ok()?);
    if date < start || date > end {
        return None;
    }
    let date_part = date_part.replace(['.', '-'], "/");
    parse_datetime(&format!("{year:04}/{date_part}{time_part}"))
}

/// 月名・米国式の日時文字列を日時へ変換する
///
/// 日付は `Nov 15, 2023`・`November 15, 2023`・`Nov 15 2023`・`15 Nov 2023`・`11/15/2023` (月/日/年) を受け付け、
/// 時刻は `22:13`・`22:13:20`・`10:13 PM`・`10:13:20 PM` を続けて書ける (日付との区切りはスペースまたは `, `)
/// 年は 4 桁に限る。タイムゾーンはローカルタイムゾーンとして扱う
///
/// # Arguments
/// * `input` - 日時文字列
///
/// # Returns
/// * `Option<Moment>` - 変換成功時は日時、失敗時は `None`
fn parse_western(input: &str) -> Option<Moment> {
    // 数字を含まない行 (ほとんどの通常のテキスト) は書式を試さずに除外する
    if !input.bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    let naive = WESTERN_DATE_FORMATS
        .iter()
        .find_map(|date_format| {
            Date::strptime(date_format, input)
                .ok()
                .map(|date| date.at(0, 0, 0, 0))
                .or_else(|| {
                    [" ", ", "].iter().find_map(|separator| {
                        WESTERN_TIME_FORMATS.iter().find_map(|time_format| {
                            DateTime::strptime(
                                format!("{date_format}{separator}{time_format}"),
                                input,
                            )
                            .ok()
                        })
                    })
                })
        })
        .filter(|naive| (1000..=9999).contains(&naive.year()))?;
    let has_millis = naive.subsec_nanosecond() / 1_000_000 != 0;
    local_moment(naive, has_millis)
}

/// `YYYY-MM-DD` または `YYYY/MM/DD` 形式の日付を解析する
///
/// # Arguments
/// * `input` - 日付文字列
///
/// # Returns
/// * `Option<Date>` - 解析成功時は日付、失敗時は `None`
fn parse_date(input: &str) -> Option<Date> {
    // 年は 4 桁に限る (`24-1-5` のような曖昧な入力を日付として扱わない)
    if input.find(['-', '/'])? != 4 {
        return None;
    }
    Date::strptime("%Y-%m-%d", input)
        .or_else(|_| Date::strptime("%Y/%m/%d", input))
        .ok()
}

/// タイムゾーンのオフセット (`Z`・`+09:00`・`+0900`・`+09`) を解析する
///
/// # Arguments
/// * `input` - オフセット文字列 (先頭が `Z`・`+`・`-` のいずれか)
///
/// # Returns
/// * `Option<Offset>` - 解析成功時はオフセット、失敗時は `None`
fn parse_offset(input: &str) -> Option<Offset> {
    if input.eq_ignore_ascii_case("z") {
        return Some(Offset::UTC);
    }
    let (sign, digits) = match input.as_bytes().first()? {
        b'+' => (1, &input[1..]),
        b'-' => (-1, &input[1..]),
        _ => return None,
    };
    if !digits.is_ascii() {
        return None;
    }
    let (hours, minutes) = match digits.len() {
        2 => (digits, "00"),
        4 => digits.split_at(2),
        5 if digits.as_bytes()[2] == b':' => (&digits[..2], &digits[3..]),
        _ => return None,
    };
    if !hours
        .bytes()
        .chain(minutes.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let (hours, minutes): (i32, i32) = (hours.parse().ok()?, minutes.parse().ok()?);
    // jiff は ±25:59 まで許すが、オフセットは 24 時間未満に限る
    if hours >= 24 || minutes >= 60 {
        return None;
    }
    Offset::from_seconds(sign * (hours * 3600 + minutes * 60)).ok()
}

// ======================================================================
// 日時 → 日時文字列変換
// ======================================================================
/// 日時をローカル日時文字列に変換する
///
/// 複数行のテキストは行ごとに変換し、変換できない行は元のまま残す
/// 入力の形式は行ごとに自動で判定する (`parse_moment` を参照)
/// ミリ秒を持つ日時は小数点以下 3 桁のミリ秒を付けて出力する
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "YYYY-MM-DD HH:MM:SS" 形式 (ミリ秒は "YYYY-MM-DD HH:MM:SS.mmm" 形式) の日時文字列。変換失敗時は元の文字列を返す
pub fn to_datetime_string(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Local)
}

/// 日時を UTC の日時文字列に変換する
///
/// 変換規則は `to_datetime_string` と同じで、タイムゾーンだけが UTC になる
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "YYYY-MM-DD HH:MM:SS" 形式 (ミリ秒は "YYYY-MM-DD HH:MM:SS.mmm" 形式) の UTC 日時文字列。変換失敗時は元の文字列を返す
pub fn to_datetime_string_utc(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Utc)
}

/// 日時をローカルタイムゾーンの ISO 8601 (RFC 3339) 形式に変換する
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "2023-11-15T07:13:20+09:00" 形式 (ミリ秒は "2023-11-15T07:13:20.123+09:00" 形式) の文字列。変換失敗時は元の文字列を返す
pub fn to_iso8601(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Iso8601Local)
}

/// 日時を UTC の ISO 8601 (RFC 3339) 形式に変換する
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "2023-11-14T22:13:20Z" 形式 (ミリ秒は "2023-11-14T22:13:20.123Z" 形式) の文字列。変換失敗時は元の文字列を返す
pub fn to_iso8601_utc(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Iso8601Utc)
}

/// 日時をローカルタイムゾーンの RFC 2822 形式に変換する
///
/// RFC 2822 は小数点以下の秒を表せないため、ミリ秒は切り捨てる
/// 年が 0〜9999 の範囲外になる日時も表せないため、その行は変換しない
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "Wed, 15 Nov 2023 07:13:20 +0900" 形式の文字列。変換失敗時は元の文字列を返す
pub fn to_rfc2822(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Rfc2822)
}

/// 日時をローカルタイムゾーンの和文形式に変換する
///
/// ミリ秒は切り捨てる
///
/// # Arguments
/// * `input` - 日時を表す文字列 (形式は行ごとに自動で判定する)
///
/// # Returns
/// * `Cow<'_, str>` - "2023年11月15日 07時13分20秒" 形式の文字列。変換失敗時は元の文字列を返す
pub fn to_japanese_datetime(input: &str) -> Cow<'_, str> {
    convert_timestamps(input, OutputStyle::Japanese)
}

// ======================================================================
// 日時 → Unixタイムスタンプ変換
// ======================================================================
/// 日時を Unixタイムスタンプに変換する
///
/// 複数行のテキストは行ごとに変換し、変換できない行は元のまま残す
/// 入力の形式は自動で判定する。受け付ける形式は ISO 8601 系 (`parse_datetime` を参照)・
/// RFC 2822 (`Wed, 15 Nov 2023 07:13:20 +0900`)・和文形式 (`2023年11月15日 07時13分20秒`)・
/// 和暦 (`令和5年11月15日`・`R5.11.15`)・月名と米国式 (`Nov 15, 2023`・`11/15/2023`)・
/// Excel のシリアル値 (`45245.5`)
/// Unixタイムスタンプの数値はすでにタイムスタンプなので変換しない
/// 秒に小数点以下がある日時はミリ秒のタイムスタンプになる
///
/// # Arguments
/// * `input` - "YYYY-MM-DD HH:MM:SS" 形式などの日時文字列
///
/// # Returns
/// * `Cow<'_, str>` - Unixタイムスタンプを表す文字列。変換失敗時は元の文字列を返す
pub fn to_unix_timestamp(input: &str) -> Cow<'_, str> {
    super::utils::process_lines(input, |line| {
        parse_moment(line)
            .filter(|moment| !moment.is_unix_number)
            .map(|moment| Cow::Owned(moment.timestamp().to_string()))
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// Unixタイムスタンプをローカル日時文字列へ変換する
    fn format_timestamp(timestamp: i64) -> Option<String> {
        moment_from_unix(timestamp).and_then(|moment| format_moment(moment, OutputStyle::Local))
    }

    /// 日時文字列を Unixタイムスタンプ (秒またはミリ秒) へ変換する
    fn parse_timestamp(input: &str) -> Option<i64> {
        parse_datetime(input).map(Moment::timestamp)
    }

    /// 有効な Unix タイムスタンプをローカル日時文字列に変換できるようにすること
    #[test]
    fn test_to_datetime_string_success() {
        let timestamp = 1_672_531_200;
        let expected = format_timestamp(timestamp).expect("タイムスタンプ変換に失敗");
        assert_eq!(to_datetime_string("1672531200"), expected);
    }

    /// ミリ秒 (13 桁) のタイムスタンプはミリ秒を保って変換すること
    #[test]
    fn test_to_datetime_string_millis() {
        let output = to_datetime_string("1700000000123");
        assert_eq!(
            output,
            format_timestamp(1_700_000_000_123).expect("タイムスタンプ変換に失敗")
        );
        assert!(output.ends_with(".123"), "output={output}");
        // ミリ秒が 0 でも小数点以下 3 桁を付ける
        assert!(to_datetime_string("1700000000000").ends_with(".000"));
        // 負のミリ秒も小数点以下を正の値で表す
        assert!(to_datetime_string("-1700000000001").ends_with(".999"));
        // 秒の最大桁 (10 桁) は秒のまま扱う
        assert_eq!(
            to_datetime_string("9999999999"),
            format_timestamp(9_999_999_999).expect("タイムスタンプ変換に失敗")
        );
    }

    /// 不正なタイムスタンプは元の文字列を返すようにすること
    #[test]
    fn test_to_datetime_string_invalid() {
        assert_eq!(to_datetime_string("invalid_timestamp"), "invalid_timestamp");
        assert!(matches!(
            to_datetime_string("invalid_timestamp"),
            Cow::Borrowed(_)
        ));
    }

    /// 複数行のタイムスタンプを行ごとに変換し、変換できない行と改行コードを保つようにすること
    #[test]
    fn test_to_datetime_string_multiline() {
        let first = format_timestamp(1_672_531_200).expect("タイムスタンプ変換に失敗");
        let second = format_timestamp(1_700_000_000_123).expect("タイムスタンプ変換に失敗");
        assert_eq!(
            to_datetime_string("1672531200\r\nmemo\n\n  1700000000123  \n"),
            format!("{first}\r\nmemo\n\n{second}\n")
        );
    }

    /// ローカル日時文字列を Unix タイムスタンプに変換できるようにすること
    #[test]
    fn test_to_unix_timestamp_success() {
        let sample = format_timestamp(1_672_531_200).expect("タイムスタンプ変換に失敗");
        let expected = parse_timestamp(&sample)
            .expect("日時パースに失敗")
            .to_string();

        assert_eq!(to_unix_timestamp(&sample), expected);
    }

    /// 不正な日時文字列は元の文字列を返すようにすること
    #[test]
    fn test_to_unix_timestamp_invalid() {
        for input in [
            "not-a-datetime",
            "2024-13-01 00:00:00",
            "2024-01-01 25:00:00",
            "24-1-5",
            "2024-01-01 10:00:00 +25:00",
            "2024-01-01 10:00:00 +09:60",
            "2024-01-01T10:00:00.",
            "2024-01/01",
            "1672531200",
            "",
        ] {
            let output = to_unix_timestamp(input);
            assert_eq!(output, input);
            assert!(matches!(output, Cow::Borrowed(_)), "input={input}");
        }
    }

    /// ローカルタイムゾーンでタイムスタンプと日時の往復変換が一致すること
    #[test]
    fn test_local_roundtrip() {
        let timestamp = 1_700_000_000_i64;
        let formatted = format_timestamp(timestamp).expect("タイムスタンプ変換に失敗");
        let parsed = parse_timestamp(&formatted).expect("日時パースに失敗");
        assert_eq!(parsed, timestamp);
    }

    /// ミリ秒のタイムスタンプも日時を経由して元の値に戻るようにすること
    #[test]
    fn test_local_roundtrip_millis() {
        for timestamp in ["1700000000123", "1700000000000", "-1700000000001"] {
            let datetime = to_datetime_string(timestamp);
            assert_eq!(to_unix_timestamp(&datetime), timestamp);
        }
    }

    /// ISO 8601 のタイムゾーン付き日時を UTC 基準のタイムスタンプに変換すること
    #[test]
    fn test_to_unix_timestamp_iso8601_offset() {
        // 2023-11-14T22:13:20Z
        for input in [
            "2023-11-14T22:13:20Z",
            "2023-11-14T22:13:20z",
            "2023-11-14T22:13:20+00:00",
            "2023-11-15T07:13:20+09:00",
            "2023-11-15 07:13:20 +0900",
            "2023-11-15T07:13:20+09",
            "2023-11-14T17:13:20-05:00",
            "2023/11/15 07:13:20+09:00",
        ] {
            assert_eq!(to_unix_timestamp(input), "1700000000", "input={input}");
        }
    }

    /// 小数点以下の秒を持つ日時はミリ秒のタイムスタンプに変換し、4 桁目以降を切り捨てるようにすること
    #[test]
    fn test_to_unix_timestamp_fraction() {
        assert_eq!(
            to_unix_timestamp("2023-11-14T22:13:20.123Z"),
            "1700000000123"
        );
        assert_eq!(
            to_unix_timestamp("2023-11-14T22:13:20.5+00:00"),
            "1700000000500"
        );
        assert_eq!(
            to_unix_timestamp("2023-11-14T22:13:20.123456789Z"),
            "1700000000123"
        );
        assert_eq!(
            to_unix_timestamp("2023-11-14T22:13:20.000Z"),
            "1700000000000"
        );
    }

    /// `T` 区切り・`/` 区切り・秒の省略・日付のみの入力をローカル日時として扱うようにすること
    #[test]
    fn test_to_unix_timestamp_flexible_local_formats() {
        let expected = parse_timestamp("2024-01-05 09:30:00").expect("日時パースに失敗");
        for input in [
            "2024-01-05T09:30:00",
            "2024-01-05t09:30:00",
            "2024/01/05 09:30:00",
            "2024/1/5 9:30:00",
            "2024-01-05 09:30",
            "  2024-01-05   09:30:00  ",
        ] {
            assert_eq!(
                to_unix_timestamp(input),
                expected.to_string(),
                "input={input}"
            );
        }

        let midnight = parse_timestamp("2024-01-05 00:00:00").expect("日時パースに失敗");
        assert_eq!(to_unix_timestamp("2024-01-05"), midnight.to_string());
        assert_eq!(to_unix_timestamp("2024/01/05"), midnight.to_string());
    }

    /// 複数行の日時を行ごとに変換し、変換できない行と改行コードを保つようにすること
    #[test]
    fn test_to_unix_timestamp_multiline() {
        assert_eq!(
            to_unix_timestamp("2023-11-14T22:13:20Z\r\nmemo\n2023-11-14T22:13:21.5Z\n"),
            "1700000000\r\nmemo\n1700000001500\n"
        );
    }

    /// UTC の日時文字列と ISO 8601 に変換し、ミリ秒を小数点以下 3 桁で保つようにすること
    #[test]
    fn test_timestamp_to_utc_formats() {
        assert_eq!(to_datetime_string_utc("1700000000"), "2023-11-14 22:13:20");
        assert_eq!(
            to_datetime_string_utc("1700000000123"),
            "2023-11-14 22:13:20.123"
        );
        assert_eq!(to_iso8601_utc("1700000000"), "2023-11-14T22:13:20Z");
        assert_eq!(to_iso8601_utc("1700000000123"), "2023-11-14T22:13:20.123Z");
        // 変換できない行と改行コードは保つ
        assert_eq!(
            to_iso8601_utc("1700000000\r\nmemo\n"),
            "2023-11-14T22:13:20Z\r\nmemo\n"
        );
    }

    /// ローカルタイムゾーンの ISO 8601 がオフセット付きで出力され、元の値に戻るようにすること
    #[test]
    fn test_to_iso8601_local() {
        for timestamp in ["1700000000", "1700000000123", "-1700000000001"] {
            let output = to_iso8601(timestamp);
            assert!(output.contains('T'), "output={output}");
            assert!(
                output.ends_with('Z') || output[output.len() - 6..].contains(['+', '-']),
                "output={output}"
            );
            assert_eq!(to_unix_timestamp(&output), timestamp);
        }
    }

    /// RFC 2822 に変換し、ミリ秒を切り捨てて秒の値に戻るようにすること
    #[test]
    fn test_to_rfc2822_roundtrip() {
        let output = to_rfc2822("1700000000");
        assert!(output.contains(", "), "output={output}");
        assert_eq!(to_unix_timestamp(&output), "1700000000");
        assert_eq!(to_rfc2822("1700000000123"), to_rfc2822("1700000000"));
        // 固定の入力はオフセットを加味した UTC 基準で戻す
        assert_eq!(
            to_unix_timestamp("Wed, 15 Nov 2023 07:13:20 +0900"),
            "1700000000"
        );
        assert_eq!(
            to_unix_timestamp("Tue, 14 Nov 2023 22:13:20 GMT"),
            "1700000000"
        );
    }

    /// 年が 0〜9999 の範囲外になる日時は RFC 2822 に変換せず、元の文字列を返すこと
    ///
    /// 実在するローカルタイムゾーン (UTC-11〜UTC+14) のどれでも範囲外になる入力だけを使う
    #[test]
    fn test_to_rfc2822_out_of_range_year() {
        for input in ["-62167300000", "-99999999999", "9999-12-31 23:59:59-12:00"] {
            let output = to_rfc2822(input);
            assert_eq!(output, input);
            assert!(matches!(output, Cow::Borrowed(_)), "input={input}");
        }
        // 範囲外の行だけを残し、他の行は変換する
        assert_eq!(
            to_rfc2822("-62167300000\n1700000000"),
            format!("-62167300000\n{}", to_rfc2822("1700000000"))
        );
    }

    /// 和文形式に変換し、ミリ秒を切り捨てて秒の値に戻るようにすること
    #[test]
    fn test_timestamp_to_japanese_roundtrip() {
        let output = to_japanese_datetime("1700000000");
        assert!(
            output.contains('年') && output.ends_with('秒'),
            "output={output}"
        );
        assert_eq!(to_unix_timestamp(&output), "1700000000");
        assert_eq!(
            to_japanese_datetime("1700000000123"),
            to_japanese_datetime("1700000000")
        );
    }

    /// 和文形式は秒や時刻の省略を許し、ローカル日時として扱うようにすること
    #[test]
    fn test_to_unix_timestamp_japanese_flexible() {
        let expected = parse_timestamp("2024-01-05 09:30:00").expect("日時パースに失敗");
        for input in ["2024年1月5日 9時30分00秒", "2024年01月05日 09時30分"] {
            assert_eq!(
                to_unix_timestamp(input),
                expected.to_string(),
                "input={input}"
            );
        }
        let midnight = parse_timestamp("2024-01-05").expect("日時パースに失敗");
        assert_eq!(to_unix_timestamp("2024年1月5日"), midnight.to_string());
        for input in ["2024年13月5日", "2024年1月5日 25時00分", "2024年1月"] {
            assert_eq!(to_unix_timestamp(input), input, "input={input}");
        }
    }

    /// 秒・ミリ秒・マイクロ秒・ナノ秒のタイムスタンプを桁数で判定し、ミリ秒より細かい桁を切り捨てるようにすること
    #[test]
    fn test_timestamp_unit_detection() {
        for (input, expected) in [
            ("1700000000", "2023-11-14 22:13:20"),
            ("1700000000123", "2023-11-14 22:13:20.123"),
            ("1700000000123456", "2023-11-14 22:13:20.123"),
            ("1700000000123456789", "2023-11-14 22:13:20.123"),
            ("1700000000.5", "2023-11-14 22:13:20.500"),
            ("-1700000000.5", "1916-02-18 01:46:39.500"),
            ("+1700000000", "2023-11-14 22:13:20"),
        ] {
            assert_eq!(to_datetime_string_utc(input), expected, "input={input}");
        }
        // マイクロ秒・ナノ秒を日時文字列にしてもミリ秒までの値に戻る
        assert_eq!(
            to_unix_timestamp(&to_datetime_string("1700000000123456")),
            "1700000000123"
        );
    }

    /// Excel のシリアル値を日付と時刻のローカル日時として変換すること
    #[test]
    fn test_excel_serial() {
        let date = parse_timestamp("2023-11-15").expect("日時パースに失敗");
        assert_eq!(to_unix_timestamp("45245.0"), date.to_string());

        // 小数部は時刻 (0.5 = 正午)
        let noon = parse_timestamp("2023-11-15 12:00:00").expect("日時パースに失敗");
        assert_eq!(to_unix_timestamp("45245.5"), noon.to_string());
        // 浮動小数点で書き出された値は最も近いミリ秒に丸める
        let afternoon = parse_timestamp("2023-11-15 14:30:00").expect("日時パースに失敗");
        assert_eq!(
            to_unix_timestamp("45245.604166666664"),
            afternoon.to_string()
        );

        // 日時文字列への変換でも同じ日時になる
        assert_eq!(to_datetime_string("45245.5"), "2023-11-15 12:00:00");

        // 範囲の端: 1900-03-01 と 9999-12-31
        assert_eq!(to_datetime_string("61.0"), "1900-03-01 00:00:00");
        assert_eq!(to_datetime_string("2958465.0"), "9999-12-31 00:00:00");
        // 整数だけの値は範囲内でも Unix タイムスタンプとして扱う
        assert_eq!(to_datetime_string_utc("45245"), "1970-01-01 12:34:05");
        assert_eq!(to_datetime_string_utc("2024"), "1970-01-01 00:33:44");
        assert_eq!(to_datetime_string_utc("61"), "1970-01-01 00:01:01");
        assert_eq!(to_datetime_string_utc("2958465"), "1970-02-04 05:47:45");
        // 範囲外の小数は Unix タイムスタンプのまま扱う
        assert_eq!(to_datetime_string_utc("60.0"), "1970-01-01 00:01:00.000");
    }

    /// 和暦を元号名・略号・元年の表記から日時へ変換すること
    #[test]
    fn test_japanese_era() {
        let date = parse_timestamp("2023-11-15").expect("日時パースに失敗");
        let datetime = parse_timestamp("2023-11-15 07:13:20").expect("日時パースに失敗");
        for input in [
            "令和5年11月15日",
            "R5.11.15",
            "r5/11/15",
            "R5-11-15",
            "平成35年11月15日",
        ] {
            assert_eq!(to_unix_timestamp(input), date.to_string(), "input={input}");
        }
        for input in [
            "令和5年11月15日 07時13分20秒",
            "令和5年11月15日 07:13:20",
            "R5.11.15 07:13:20",
            "R5.11.15T07:13:20",
        ] {
            assert_eq!(
                to_unix_timestamp(input),
                datetime.to_string(),
                "input={input}"
            );
        }

        // 各元号の元年・最終年と元年の表記
        for (input, expected) in [
            ("令和元年5月1日", "2019-05-01"),
            ("H30.12.31", "2018-12-31"),
            ("昭和64年1月7日", "1989-01-07"),
            ("大正15年12月25日", "1926-12-25"),
            ("明治45年7月30日", "1912-07-30"),
            ("S45.11.25", "1970-11-25"),
        ] {
            assert_eq!(
                to_unix_timestamp(input),
                parse_timestamp(expected)
                    .expect("日時パースに失敗")
                    .to_string(),
                "input={input}"
            );
        }

        for input in [
            "令和0年1月1日",
            "令和年1月1日",
            "令和5年13月1日",
            "R5",
            "R5.11",
            "Hello",
            // バージョン番号・型番 (月日が 1 桁)
            "R4.3.1",
            "M1.2.3",
            // 元号の期間外
            "R1.04.30",
            "H31.05.01",
            "S64.01.08",
            "T15.12.25",
            "M45.07.30",
        ] {
            assert_eq!(to_unix_timestamp(input), input, "input={input}");
        }
    }

    /// 月名・米国式の日時をローカル日時として変換すること
    #[test]
    fn test_western_formats() {
        let date = parse_timestamp("2023-11-15").expect("日時パースに失敗");
        for input in [
            "Nov 15, 2023",
            "November 15, 2023",
            "nov 15 2023",
            "15 Nov 2023",
            "15 November 2023",
            "11/15/2023",
        ] {
            assert_eq!(to_unix_timestamp(input), date.to_string(), "input={input}");
        }

        let evening = parse_timestamp("2023-11-15 22:13:20").expect("日時パースに失敗");
        for input in [
            "Nov 15, 2023 22:13:20",
            "Nov 15, 2023, 10:13:20 PM",
            "15 Nov 2023 10:13:20 pm",
            "11/15/2023 22:13:20",
            "11/15/2023 10:13:20 PM",
        ] {
            assert_eq!(
                to_unix_timestamp(input),
                evening.to_string(),
                "input={input}"
            );
        }
        let minute = parse_timestamp("2023-11-15 07:13").expect("日時パースに失敗");
        assert_eq!(to_unix_timestamp("11/15/2023 7:13 AM"), minute.to_string());

        // 月が 13 以上・年が 4 桁でない・月名が不正な入力は変換しない
        for input in [
            "15/11/2023",
            "11/15/23",
            "Foo 15, 2023",
            "Nov 32, 2023",
            "Nov 15",
        ] {
            assert_eq!(to_unix_timestamp(input), input, "input={input}");
        }
    }

    /// どの入力形式でも、モードに応じた出力形式へ変換すること
    #[test]
    fn test_mixed_inputs_to_output_formats() {
        // 同じ時刻 (2023-11-14T22:13:20Z) を表す UTC 基準の入力
        for (input, expected) in [
            ("1700000000", "2023-11-14T22:13:20Z"),
            ("1700000000000", "2023-11-14T22:13:20.000Z"),
            ("1700000000000000", "2023-11-14T22:13:20.000Z"),
            ("2023-11-14T22:13:20Z", "2023-11-14T22:13:20Z"),
            ("2023-11-15T07:13:20+09:00", "2023-11-14T22:13:20Z"),
            ("Tue, 14 Nov 2023 22:13:20 GMT", "2023-11-14T22:13:20Z"),
        ] {
            assert_eq!(to_iso8601_utc(input), expected, "input={input}");
        }

        // 複数行に異なる形式が混在しても行ごとに判定し、変換できない行と改行コードを保つ
        assert_eq!(
            to_iso8601_utc(
                "1700000000

2023-11-15T07:13:21+09:00
memo
"
            ),
            "2023-11-14T22:13:20Z

2023-11-14T22:13:21Z
memo
"
        );
    }
}

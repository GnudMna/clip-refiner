//! 登録クリップのテンプレート展開
//!
//! `{date}` などのプレースホルダをコピー時の日時・クリップボード・ユーザー名などへ置き換える
//! 本文は借用したまま走査し、平文の断片を中間データとして複製しない

use std::fmt::Write as _;

use jiff::ZonedDateTime;
use jiff::fmt::strtime;
use thiserror::Error;

use crate::consts::MAX_CLIPBOARD_TEXT_BYTES;
use crate::security::format_mib;

// ======================================================================
// 定数
// ======================================================================
/// `{date}` の既定書式
const DEFAULT_DATE_FORMAT: &str = "%Y-%m-%d";
/// `{time}` の既定書式
const DEFAULT_TIME_FORMAT: &str = "%H:%M:%S";
/// `{datetime}` の既定書式
const DEFAULT_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
/// `{weekday}` の曜日表記 (月曜始まり)
const WEEKDAY_LABELS: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

// ======================================================================
// 公開型
// ======================================================================
/// テンプレート展開に使う値
///
/// 展開 1 回につき日時を 1 度だけ取得し、同じテンプレート内の日時を揃える
#[derive(Debug, Clone)]
pub struct ClipTemplateContext<'a> {
    /// `{date}` / `{time}` / `{datetime}` / `{weekday}` の基準日時
    pub now: ZonedDateTime,
    /// `{clipboard}` に差し込むテキスト。`None` は空文字として扱う
    pub clipboard: Option<&'a str>,
    /// `{user}` に差し込むユーザー名。`None` は空文字として扱う
    pub user_name: Option<&'a str>,
    /// `{counter}` に差し込む連番 (今回のコピーで払い出す値)
    pub counter: u64,
}

/// テンプレート展開の失敗理由
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClipTemplateError {
    /// 日時の書式指定が不正
    #[error("テンプレートの日時書式が不正です: {0}")]
    InvalidFormat(String),
    /// 展開結果がクリップボード本文の上限を超える
    #[error(
        "テンプレートの展開結果がサイズ上限 ({}) を超えています",
        format_mib(MAX_CLIPBOARD_TEXT_BYTES)
    )]
    TooLarge,
    /// `{uuid}` 用の乱数を取得できない
    #[error("UUID の生成に失敗しました")]
    Uuid,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// テンプレートを展開する
///
/// 対応するプレースホルダは `{date}` `{time}` `{datetime}` (`{date:%Y/%m/%d}` のように
/// jiff の strftime 書式を指定可)、`{weekday}`、`{clipboard}`、`{uuid}`、`{user}`、`{counter}`
/// `{counter}` は 1 回の展開の中ではすべて同じ値になる
/// `{{` / `}}` は `{` / `}` になる。未知の名前や閉じていない `{` は書かれたまま残す
///
/// # Arguments
/// * `source` - 展開するテンプレート本文
/// * `ctx` - 展開に使う値
/// * `uuid_bytes` - `{uuid}` 1 個ごとに呼ぶ 16 バイトの乱数源。失敗時は `None`
///
/// # Returns
/// * `Result<String, ClipTemplateError>` - 展開後のテキスト
///
/// # Errors
/// 日時書式が不正な場合、展開結果が上限を超える場合、乱数を取得できない場合
pub fn expand_clip_template(
    source: &str,
    ctx: &ClipTemplateContext<'_>,
    mut uuid_bytes: impl FnMut() -> Option<[u8; 16]>,
) -> Result<String, ClipTemplateError> {
    let mut out = String::with_capacity(source.len());
    for token in Tokens::new(source) {
        match token {
            Token::Literal(text) => out.push_str(text),
            Token::Placeholder(placeholder) => {
                render_placeholder(&mut out, placeholder, ctx, &mut uuid_bytes)?;
            }
        }
        if out.len() > MAX_CLIPBOARD_TEXT_BYTES {
            return Err(ClipTemplateError::TooLarge);
        }
    }
    Ok(out)
}

/// テンプレートが `{clipboard}` を含むか判定する
///
/// 不要なときにクリップボード本文を読まないために使う
///
/// # Arguments
/// * `source` - 判定するテンプレート本文
///
/// # Returns
/// * `bool` - `{clipboard}` を含む場合は `true`
pub fn clip_template_uses_clipboard(source: &str) -> bool {
    Tokens::new(source).any(|token| matches!(token, Token::Placeholder(Placeholder::Clipboard)))
}

/// テンプレートが `{counter}` を含むか判定する
///
/// 連番を進めるのは `{counter}` を含むテンプレートをコピーしたときだけにするために使う
///
/// # Arguments
/// * `source` - 判定するテンプレート本文
///
/// # Returns
/// * `bool` - `{counter}` を含む場合は `true`
pub fn clip_template_uses_counter(source: &str) -> bool {
    Tokens::new(source).any(|token| matches!(token, Token::Placeholder(Placeholder::Counter)))
}

// ======================================================================
// 字句解析
// ======================================================================
/// テンプレートを構成する要素
#[derive(Debug, PartialEq, Eq)]
enum Token<'a> {
    /// そのまま出力する文字列
    Literal(&'a str),
    /// 展開するプレースホルダ
    Placeholder(Placeholder<'a>),
}

/// 対応するプレースホルダ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placeholder<'a> {
    /// 日付 (書式指定なしは `DEFAULT_DATE_FORMAT`)
    Date(Option<&'a str>),
    /// 時刻 (書式指定なしは `DEFAULT_TIME_FORMAT`)
    Time(Option<&'a str>),
    /// 日時 (書式指定なしは `DEFAULT_DATETIME_FORMAT`)
    DateTime(Option<&'a str>),
    /// 曜日 (`月` など)
    Weekday,
    /// 展開時点のクリップボードのテキスト
    Clipboard,
    /// ランダムな UUID (v4)
    Uuid,
    /// Windows のユーザー名
    User,
    /// コピーのたびに 1 ずつ増える連番
    Counter,
}

/// テンプレート本文を先頭から `Token` に分割するイテレータ
struct Tokens<'a> {
    /// 未処理の残り
    rest: &'a str,
}

impl<'a> Tokens<'a> {
    fn new(source: &'a str) -> Self {
        Self { rest: source }
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.rest;
        if rest.is_empty() {
            return None;
        }

        // `{{` / `}}` は 1 文字のリテラルにする
        if rest.starts_with("{{") || rest.starts_with("}}") {
            self.rest = &rest[2..];
            return Some(Token::Literal(&rest[..1]));
        }

        if rest.starts_with('{')
            && let Some((placeholder, len)) = parse_placeholder(rest)
        {
            self.rest = &rest[len..];
            return Some(Token::Placeholder(placeholder));
        }

        // 次の `{` / `}` の直前までをリテラルにする (先頭の解釈できない 1 文字は含める)
        let first_len = rest.chars().next().map_or(1, char::len_utf8);
        let end = rest[first_len..]
            .find(['{', '}'])
            .map_or(rest.len(), |offset| offset + first_len);
        self.rest = &rest[end..];
        Some(Token::Literal(&rest[..end]))
    }
}

/// 先頭の `{name}` / `{name:arg}` をプレースホルダとして解釈する
///
/// # Arguments
/// * `rest` - `{` で始まる文字列
///
/// # Returns
/// * `Option<(Placeholder, usize)>` - プレースホルダと消費したバイト数を返し、
///   閉じていない・未知の名前・引数を取らない名前への引数指定は `None` を返す
fn parse_placeholder(rest: &str) -> Option<(Placeholder<'_>, usize)> {
    let close = rest.find('}')?;
    let inner = &rest[1..close];
    let (name, arg) = match inner.split_once(':') {
        Some((name, arg)) => (name, Some(arg)),
        None => (inner, None),
    };
    let placeholder = match (name, arg) {
        ("date", arg) => Placeholder::Date(arg),
        ("time", arg) => Placeholder::Time(arg),
        ("datetime", arg) => Placeholder::DateTime(arg),
        ("weekday", None) => Placeholder::Weekday,
        ("clipboard", None) => Placeholder::Clipboard,
        ("uuid", None) => Placeholder::Uuid,
        ("user", None) => Placeholder::User,
        ("counter", None) => Placeholder::Counter,
        _ => return None,
    };
    Some((placeholder, close + 1))
}

// ======================================================================
// 展開
// ======================================================================
/// プレースホルダ 1 個を展開して `out` へ追記する
fn render_placeholder(
    out: &mut String,
    placeholder: Placeholder<'_>,
    ctx: &ClipTemplateContext<'_>,
    uuid_bytes: &mut impl FnMut() -> Option<[u8; 16]>,
) -> Result<(), ClipTemplateError> {
    match placeholder {
        Placeholder::Date(format) => {
            write_datetime(out, &ctx.now, format.unwrap_or(DEFAULT_DATE_FORMAT))
        }
        Placeholder::Time(format) => {
            write_datetime(out, &ctx.now, format.unwrap_or(DEFAULT_TIME_FORMAT))
        }
        Placeholder::DateTime(format) => {
            write_datetime(out, &ctx.now, format.unwrap_or(DEFAULT_DATETIME_FORMAT))
        }
        Placeholder::Weekday => {
            let index = usize::from(ctx.now.weekday().to_monday_zero_offset().unsigned_abs());
            out.push_str(WEEKDAY_LABELS[index]);
            Ok(())
        }
        Placeholder::Clipboard => {
            out.push_str(ctx.clipboard.unwrap_or_default());
            Ok(())
        }
        Placeholder::User => {
            out.push_str(ctx.user_name.unwrap_or_default());
            Ok(())
        }
        Placeholder::Counter => {
            let _ = write!(out, "{}", ctx.counter);
            Ok(())
        }
        Placeholder::Uuid => {
            let bytes = uuid_bytes().ok_or(ClipTemplateError::Uuid)?;
            write_uuid_v4(out, bytes);
            Ok(())
        }
    }
}

/// 日時を strftime 書式で追記する
///
/// 書式の解析エラーや書式化できない指定子は `InvalidFormat` にする
/// 書式化に失敗した場合は途中の出力を残さない
fn write_datetime(
    out: &mut String,
    now: &ZonedDateTime,
    format: &str,
) -> Result<(), ClipTemplateError> {
    let invalid = || ClipTemplateError::InvalidFormat(format.to_string());
    if format.is_empty() {
        return Err(invalid());
    }
    let formatted = strtime::format(format, now).map_err(|_| invalid())?;
    out.push_str(&formatted);
    Ok(())
}

/// 乱数 16 バイトから UUID v4 の文字列 (小文字 16 進) を追記する
fn write_uuid_v4(out: &mut String, mut bytes: [u8; 16]) {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        let _ = write!(out, "{byte:02x}");
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use jiff::tz::{Offset, TimeZone};

    use super::*;

    /// 2026-10-08 (木) 09:05:03 +09:00 を基準日時とするコンテキスト
    fn ctx<'a>(clipboard: Option<&'a str>, user_name: Option<&'a str>) -> ClipTemplateContext<'a> {
        let offset = Offset::from_hours(9).expect("offset");
        ClipTemplateContext {
            now: date(2026, 10, 8)
                .at(9, 5, 3, 0)
                .to_zoned_datetime(TimeZone::fixed(offset))
                .expect("datetime"),
            clipboard,
            user_name,
            counter: 7,
        }
    }

    fn expand(source: &str) -> Result<String, ClipTemplateError> {
        expand_clip_template(source, &ctx(Some("COPIED"), Some("alice")), || {
            Some([0xff; 16])
        })
    }

    /// 日付・時刻・日時・曜日を既定書式で展開すること
    #[test]
    fn expands_datetime_placeholders_with_default_format() {
        assert_eq!(
            expand("{date} {time} / {datetime} ({weekday})").expect("expand"),
            "2026-10-08 09:05:03 / 2026-10-08 09:05:03 (木)"
        );
    }

    /// 書式指定付きの日時を strftime 書式で展開すること
    #[test]
    fn expands_datetime_with_custom_format() {
        assert_eq!(
            expand("{date:%Y/%m/%d}|{time:%H時%M分}|{datetime:%Y%m%dT%H%M%S%z}").expect("expand"),
            "2026/10/08|09時05分|20261008T090503+0900"
        );
    }

    /// 曜日・月名・パディング指定・小数秒などよく使う指定子を展開できること
    #[test]
    fn expands_common_strftime_specifiers() {
        assert_eq!(
            expand("{date:%A %a %B %b}").expect("expand"),
            "Thursday Thu October Oct"
        );
        assert_eq!(expand("{date:%-m/%-d %e|}").expect("expand"), "10/8  8|");
        assert_eq!(
            expand("{time:%H:%M:%S%.3f}").expect("expand"),
            "09:05:03.000"
        );
        assert_eq!(
            expand("{datetime:%Y%m%dT%H%M%S%z}").expect("expand"),
            "20261008T090503+0900"
        );
    }

    /// 不正な書式指定や空の書式はエラーにすること
    #[test]
    fn rejects_invalid_datetime_format() {
        // 未対応の指定子と、末尾の単独の `%`
        assert_eq!(
            expand("{date:%+}"),
            Err(ClipTemplateError::InvalidFormat("%+".to_string()))
        );
        assert_eq!(
            expand("{date:%Y%}"),
            Err(ClipTemplateError::InvalidFormat("%Y%".to_string()))
        );
        assert_eq!(
            expand("{time:}"),
            Err(ClipTemplateError::InvalidFormat(String::new()))
        );
    }

    /// クリップボードとユーザー名を差し込み、未取得なら空文字にすること
    #[test]
    fn expands_clipboard_and_user() {
        assert_eq!(
            expand("[{clipboard}] by {user}").expect("expand"),
            "[COPIED] by alice"
        );

        let empty = expand_clip_template("[{clipboard}]{user}", &ctx(None, None), || None);
        assert_eq!(empty.expect("expand"), "[]");
    }

    /// UUID を v4 形式で展開し、プレースホルダごとに乱数を取り直すこと
    #[test]
    fn expands_uuid_v4_per_placeholder() {
        let mut calls = 0u8;
        let output = expand_clip_template("{uuid} {uuid}", &ctx(None, None), || {
            calls += 1;
            Some([calls; 16])
        })
        .expect("expand");

        assert_eq!(
            output,
            "01010101-0101-4101-8101-010101010101 02020202-0202-4202-8202-020202020202"
        );
    }

    /// 乱数を取得できない場合はエラーにすること
    #[test]
    fn uuid_fails_without_random_source() {
        let result = expand_clip_template("{uuid}", &ctx(None, None), || None);
        assert_eq!(result, Err(ClipTemplateError::Uuid));
    }

    /// `{{` / `}}` を波括弧 1 文字として出力すること
    #[test]
    fn escapes_double_braces() {
        assert_eq!(
            expand("{{date}} {{{date}}}").expect("expand"),
            "{date} {2026-10-08}"
        );
    }

    /// 未知の名前・閉じていない括弧・引数を取らない名前への引数は書かれたまま残すこと
    #[test]
    fn keeps_unknown_or_malformed_placeholders() {
        let source = r#"{"key": 1} {name} {user:x} {date"#;
        assert_eq!(expand(source).expect("expand"), source);
        assert_eq!(expand("a } b {").expect("expand"), "a } b {");
    }

    /// マルチバイト文字の前後でもプレースホルダを展開すること
    #[test]
    fn handles_multibyte_text() {
        assert_eq!(
            expand("日付：{date}、担当：{user}").expect("expand"),
            "日付：2026-10-08、担当：alice"
        );
    }

    /// 展開結果がクリップボード上限を超える場合はエラーにすること
    #[test]
    fn rejects_oversized_output() {
        let clipboard = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES / 2 + 1);
        let result = expand_clip_template(
            "{clipboard}{clipboard}",
            &ctx(Some(&clipboard), None),
            || None,
        );
        assert_eq!(result, Err(ClipTemplateError::TooLarge));
    }

    /// `{clipboard}` を含むかどうかを判定できること
    #[test]
    fn detects_clipboard_placeholder() {
        assert!(clip_template_uses_clipboard("前置き {clipboard}"));
        assert!(!clip_template_uses_clipboard("{{clipboard}} {date}"));
        assert!(!clip_template_uses_clipboard("{clipboard:x}"));
    }

    /// `{counter}` を 1 回の展開の中で同じ値に展開すること
    #[test]
    fn expands_counter_with_same_value() {
        assert_eq!(
            expand("No.{counter} / {counter}").expect("expand"),
            "No.7 / 7"
        );
        // 引数付きは未対応のため書かれたまま残す
        assert_eq!(expand("{counter:3}").expect("expand"), "{counter:3}");
    }

    /// `{counter}` を含むかどうかを判定できること
    #[test]
    fn detects_counter_placeholder() {
        assert!(clip_template_uses_counter("No.{counter}"));
        assert!(!clip_template_uses_counter("{{counter}} {date}"));
        assert!(!clip_template_uses_counter("{counter:x}"));
    }
}

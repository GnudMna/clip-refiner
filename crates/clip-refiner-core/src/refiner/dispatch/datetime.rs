use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::datetime as datetime_transform;

/// 日時変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::ToDatetimeString => datetime_transform::to_datetime_string(text),
        RefineMode::ToDatetimeStringUtc => datetime_transform::to_datetime_string_utc(text),
        RefineMode::ToIso8601 => datetime_transform::to_iso8601(text),
        RefineMode::ToIso8601Utc => datetime_transform::to_iso8601_utc(text),
        RefineMode::ToRfc2822 => datetime_transform::to_rfc2822(text),
        RefineMode::ToJapaneseDatetime => datetime_transform::to_japanese_datetime(text),
        RefineMode::ToUnixTimestamp => datetime_transform::to_unix_timestamp(text),
        _ => unreachable!("{mode:?} は Datetime カテゴリではない"),
    }
}

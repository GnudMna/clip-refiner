use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::datetime as datetime_transform;

/// 日時変換カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::TimestampToDatetime => datetime_transform::timestamp_to_datetime_string(text),
        RefineMode::DatetimeToTimestamp => datetime_transform::datetime_string_to_timestamp(text),
        _ => unreachable!("{mode:?} は Datetime カテゴリではない"),
    }
}

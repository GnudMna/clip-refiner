use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::base64 as base64_transform;

/// Base64 カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::Base64Encode => Cow::Owned(base64_transform::base64_encode(text)),
        RefineMode::Base64Decode => {
            base64_transform::base64_decode(text).map_or(Cow::Borrowed(text), Cow::Owned)
        }
        _ => unreachable!("{mode:?} は Base64 カテゴリではない"),
    }
}

use std::borrow::Cow;

use super::super::context::RefineContext;
use super::super::mode::RefineMode;
use super::super::transform::url as url_transform;

/// URL 操作カテゴリの加工を実行する
pub(crate) fn refine<'a>(mode: RefineMode, text: &'a str, _ctx: &RefineContext) -> Cow<'a, str> {
    match mode {
        RefineMode::UrlEncode => url_transform::url_encode(text),
        RefineMode::UrlDecode => {
            url_transform::url_decode(text).map_or_else(|_| Cow::Borrowed(text), Cow::Owned)
        }
        RefineMode::RemoveUtm => url_transform::remove_utm_params(text),
        _ => unreachable!("{mode:?} は UrlActions カテゴリではない"),
    }
}

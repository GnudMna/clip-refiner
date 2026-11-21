//! 正規表現によるテキスト変換とプレビュー
//!
//! コンパイルキャッシュ・マッチタイムアウト・実行パス・設定 UI 向けプレビューを
//! サブモジュールに分割する

mod cache;
mod match_timeout;
mod ops;
mod preview;

pub(crate) use cache::RegexPatternCache;
pub(crate) use ops::{regex_delete, regex_extract, regex_replace, regex_split};
pub use preview::{
    REGEX_PREVIEW_SAMPLE, RegexPreviewResult, preview_regex_replace, validate_regex_settings,
};

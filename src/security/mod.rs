//! 入力サイズ制限と機密情報の表示マスキングを提供するモジュール

mod limits;
mod snippet;

pub use limits::{is_within_clipboard_limit, is_within_parser_limit};
pub use snippet::format_public_snippet;

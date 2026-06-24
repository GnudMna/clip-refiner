//! クリップボード加工モードの定義と、各モードへのディスパッチを提供するモジュール
//!
//! `RefineMode` による加工処理の統合と、クリップボードへの読み書きを担当する

pub mod datetime;
pub mod escape;
pub mod json;
pub mod line_actions;
pub mod markdown;
pub mod number;
pub mod path;
pub mod trim;
pub mod url;
pub mod utils;
pub mod yaml;

mod clipboard;
mod dispatch;
mod mode;
mod ordered_value;

pub use clipboard::{ClipboardProcessError, ClipboardProcessOutcome, process_clipboard};
pub use dispatch::Refiner;
pub use mode::{RefineCategory, RefineMode};
pub use ordered_value::OrderedValue;

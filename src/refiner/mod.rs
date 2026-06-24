//! クリップボード加工モードの定義と、各モードへのディスパッチを提供するモジュール
//!
//! `RefineMode` による加工処理の統合と、クリップボードへの読み書きを担当する

mod clipboard;
mod dispatch;
mod mode;
mod ordered_value;
mod transform;

pub use clipboard::{ClipboardProcessError, ClipboardProcessOutcome, process_clipboard};
pub use dispatch::Refiner;
pub use mode::{RefineCategory, RefineMode};
pub use ordered_value::OrderedValue;

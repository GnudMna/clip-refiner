//! クリップボード加工モードの定義と、各モードへのディスパッチを提供するモジュール
//!
//! `RefineMode` による加工処理の統合と、クリップボードへの読み書きを担当する
//!
//! # Examples
//!
//! [`RefineMode`] は [`Refiner`] トレイト経由でテキストへ適用できる
//!
//! ```
//! use clip_refiner::refiner::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! assert_eq!(RefineMode::Trim.refine("  a  ", &ctx), "a");
//! ```

mod clipboard;
mod context;
mod dispatch;
mod mode;
mod ordered_value;
pub(crate) mod text_clipboard;
mod transform;

pub(crate) use text_clipboard::TextClipboard;

pub(crate) use clipboard::process_text_clipboard;
pub use clipboard::{ClipboardProcessError, ClipboardProcessOutcome, process_clipboard};
pub use context::RefineContext;
pub use dispatch::Refiner;
pub use mode::{RefineCategory, RefineMode};
pub use ordered_value::OrderedValue;

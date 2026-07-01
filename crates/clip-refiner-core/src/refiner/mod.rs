//! クリップボード加工モードの定義と、各モードへのディスパッチを提供するモジュール
//!
//! `RefineMode` による加工処理の統合と、クリップボードへの読み書きを担当する
//!
//! # Examples
//!
//! [`RefineMode`] は [`Refiner`] トレイト経由でテキストへ適用できる
//!
//! 複数モードの連鎖は [`apply_text_pipeline`] または [`apply_pipeline_to_text`] を使う
//!
//! ```
//! use clip_refiner_core::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! assert_eq!(RefineMode::Trim.refine("  a  ", &ctx), "a");
//! ```

mod clipboard;
mod context;
mod dispatch;
mod mode;
mod ordered_value;
mod pipeline;
pub mod text_clipboard;
mod transform;

pub use text_clipboard::{ImageClipboard, TextClipboard};

pub use clipboard::process_clipboard_pipeline_io;
pub use clipboard::{
    ClipboardProcessError, ClipboardProcessOutcome, apply_pipeline_to_text, process_clipboard,
    process_clipboard_pipeline,
};
pub use context::RefineContext;
pub use dispatch::Refiner;
pub use mode::{RefineCategory, RefineMode};
pub use ordered_value::OrderedValue;
pub use pipeline::{apply_text_pipeline, split_pipeline};

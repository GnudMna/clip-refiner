//! コアクレートの単体テスト用ヘルパー

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "テスト専用のため、失敗はそのまま panic させる"
)]

mod clipboard;
#[cfg(feature = "config")]
mod config_dir;

pub use clipboard::InMemoryTextClipboard;
#[cfg(feature = "config")]
pub use config_dir::with_temp_config_dir;

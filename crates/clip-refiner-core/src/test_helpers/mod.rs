//! コアクレートの単体テスト用ヘルパー

mod clipboard;
#[cfg(feature = "config")]
mod config_dir;

pub use clipboard::InMemoryTextClipboard;
#[cfg(feature = "config")]
pub use config_dir::with_temp_config_dir;

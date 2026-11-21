//! コアクレートの単体テスト用ヘルパー

mod clipboard;
mod config_dir;

pub use clipboard::InMemoryTextClipboard;
pub use config_dir::with_temp_config_dir;

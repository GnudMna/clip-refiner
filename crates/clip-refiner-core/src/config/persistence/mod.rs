//! `config.toml` の読み書きと読み込み結果型
//!
//! 初回生成・破損時フォールバック・レガシー `[[clips]]` 移行・原子的保存を担う

mod load;
mod outcome;
mod save;

pub use outcome::{ConfigDiskReload, ConfigLoadFallback, ConfigLoadOutcome, ConfigReloadError};
pub use save::disk_config_modified_time;

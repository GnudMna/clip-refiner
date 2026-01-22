use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::refiner::RefineMode;

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 実行モード
    pub mode: RefineMode,
    /// 監視周期（ミリ秒）
    pub interval_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: RefineMode::UrlDecode,
            interval_ms: 1000,
        }
    }
}

impl AppConfig {
    /// 設定ファイルのパスを取得
    fn config_path() -> Result<PathBuf> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir)
            .context("設定ディレクトリの作成に失敗しました")?;
        Ok(config_dir.join("config.json"))
    }

    /// 設定をファイルから読み込む
    pub fn load() -> Self {
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("設定ファイルパスの取得に失敗: {}", e);
                return Self::default();
            }
        };

        if !config_path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                match serde_json::from_str::<AppConfig>(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("設定ファイルの解析に失敗: {}", e);
                        Self::default()
                    }
                }
            }
            Err(e) => {
                eprintln!("設定ファイルの読み込みに失敗: {}", e);
                Self::default()
            }
        }
    }

    /// 設定をファイルに保存する
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)
            .context("設定のシリアライズに失敗しました")?;
        std::fs::write(&config_path, content)
            .context("設定ファイルの書き込みに失敗しました")?;
        Ok(())
    }
}

/// 設定ディレクトリのパスを取得
fn get_config_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA")
            .context("APPDATA環境変数の取得に失敗しました")?;
        Ok(PathBuf::from(appdata).join("ClipRefiner"))
    }

    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME")
            .context("HOME環境変数の取得に失敗しました")?;
        Ok(PathBuf::from(home).join(".config").join("clip-refiner"))
    }
}

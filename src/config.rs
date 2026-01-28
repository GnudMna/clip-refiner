use std::path::PathBuf;

use crate::notification::error::show_error_notification;
use crate::refiner::RefineMode;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// クリップボード監視モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// ポーリング方式（定期的にチェック）
    Polling,
    /// イベント方式（クリップボード変更時に即座に反応）
    #[cfg(windows)]
    Event,
}

impl Default for MonitorMode {
    fn default() -> Self {
        Self::Polling
    }
}

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 実行モード
    pub mode: RefineMode,
    /// 監視周期(ミリ秒)
    pub interval_ms: u64,
    /// 監視モード
    #[serde(default)]
    pub monitor_mode: MonitorMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: RefineMode::UrlDecode,
            interval_ms: 1000,
            monitor_mode: MonitorMode::default(),
        }
    }
}

impl AppConfig {
    /// 設定ファイルのパスを取得
    fn config_path() -> Result<PathBuf> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
        Ok(config_dir.join("config.json"))
    }

    pub fn load() -> Self {
        // 設定ファイルパス取得
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                show_error_notification("設定ファイルパスの取得に失敗", &format!("{:?}", e));
                return Self::default();
            }
        };

        // 設定ファイルが存在しない → デフォルトで継続
        if !config_path.exists() {
            return Self::default();
        }

        // ファイル読み込み
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                show_error_notification("設定ファイルの読み込みに失敗", &format!("{:?}", e));
                return Self::default();
            }
        };

        // JSON パース
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(e) => {
                show_error_notification("設定ファイルの解析に失敗", &format!("{:?}", e));
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path().map_err(|e| {
            show_error_notification("設定ファイルパスの取得に失敗", &format!("{:?}", e));
            e
        })?;

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            show_error_notification("設定のシリアライズに失敗", &format!("{:?}", e));
            e
        })?;

        std::fs::write(&config_path, content).map_err(|e| {
            show_error_notification("設定ファイルの書き込みに失敗", &format!("{:?}", e));
            e
        })?;

        Ok(())
    }
}

/// 設定ディレクトリのパスを取得
fn get_config_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").context("APPDATA環境変数の取得に失敗しました")?;
        Ok(PathBuf::from(appdata).join("ClipRefiner"))
    }

    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").context("HOME環境変数の取得に失敗しました")?;
        Ok(PathBuf::from(home).join(".config").join("clip-refiner"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.mode, RefineMode::UrlDecode);
    }

    #[test]
    fn test_app_config_serde() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.interval_ms, decoded.interval_ms);
        assert_eq!(config.mode, decoded.mode);
    }
}

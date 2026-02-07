use std::path::PathBuf;

use crate::notification::show_simple_notification;
use crate::refiner::RefineMode;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// クリップボードの監視方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// 一定間隔でクリップボードの内容を確認するポーリング方式。
    /// すべてのプラットフォームで動作する基本的な監視モードです。
    Polling,
    /// OSのクリップボード更新イベントを購読する方式（Windows専用）。
    /// 低遅延かつCPU負荷が低いのが特徴です。
    #[cfg(windows)]
    Event,
}

impl Default for MonitorMode {
    fn default() -> Self {
        Self::Polling
    }
}

/// アプリケーションの設定情報。JSONファイルとして保存・読み込みされます。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 最後に使用した（または常駐時に使用する）加工モード
    pub mode: RefineMode,
    /// 監視周期(ミリ秒)。ポーリング方式の場合に使用されます。
    pub interval_ms: u64,
    /// 使用する監視方式（Polling または Event）
    #[serde(default)]
    pub monitor_mode: MonitorMode,
    /// 履歴機能が有効かどうか
    #[serde(default)]
    pub history_enabled: bool,
    /// 成功時に通知を表示するかどうか
    #[serde(default)]
    pub show_success_notification: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: RefineMode::UrlDecode,
            interval_ms: 1000,
            monitor_mode: MonitorMode::default(),
            history_enabled: false,
            show_success_notification: false,
        }
    }
}

impl AppConfig {
    /// 設定ファイルの保存先パスをシステムOSに合わせて取得する
    ///
    /// # Returns
    /// * `Result<PathBuf>` - 設定ファイルの完全なパス。
    fn config_path() -> Result<PathBuf> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
        Ok(config_dir.join("config.json"))
    }

    /// 設定ファイルを読み込む。存在しない場合や失敗した場合はデフォルト設定を返す
    ///
    /// # Returns
    /// * `Self` - ファイルから読み込まれた `AppConfig`、またはデフォルトの `AppConfig`。
    pub fn load() -> Self {
        // 設定ファイルパス取得
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                show_simple_notification("設定ファイルパスの取得に失敗", &format!("{:?}", e));
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
                show_simple_notification("設定ファイルの読み込みに失敗", &format!("{:?}", e));
                return Self::default();
            }
        };

        // JSON パース
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(e) => {
                show_simple_notification("設定ファイルの解析に失敗", &format!("{:?}", e));
                Self::default()
            }
        }
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// # Returns
    /// * `Result<()>` - 保存が成功した場合は `Ok(())`、失敗した場合は `Err` を返す。
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path().map_err(|e| {
            show_simple_notification("設定ファイルパスの取得に失敗", &format!("{:?}", e));
            e
        })?;

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            show_simple_notification("設定のシリアライズに失敗", &format!("{:?}", e));
            e
        })?;

        std::fs::write(&config_path, content).map_err(|e| {
            show_simple_notification("設定ファイルの書き込みに失敗", &format!("{:?}", e));
            e
        })?;

        Ok(())
    }
}

/// 設定ディレクトリのパスを取得
///
/// # Returns
/// * `Result<PathBuf>` - OSに応じた設定ディレクトリのパス。
fn get_config_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").context("APPDATA環境変数の取得に失敗しました")?;
        Ok(PathBuf::from(appdata).join("ClipRefiner"))
    }

    #[cfg(not(windows))]
    {
        // XDG Base Directory Specification に従い、~/.config/clip-refiner を使用
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

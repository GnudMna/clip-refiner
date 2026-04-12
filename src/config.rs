use std::fs;
use std::path::PathBuf;

use crate::consts;
use crate::refiner::RefineMode;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ======================================================================
// 監視モード
// ======================================================================
/// クリップボードの監視方式
///
/// クリップボードの更新を検知するための異なるアプローチを提供します。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// 一定間隔でクリップボードの内容を確認するポーリング方式。
    /// すべてのプラットフォームで動作する基本的な監視モードです。
    #[default]
    Polling,
    /// OSのクリップボード更新イベントを購読する方式（Windows専用）。
    /// 低遅延かつCPU負荷が低いのが特徴です。
    #[cfg(windows)]
    Event,
}

// ======================================================================
// 通知設定
// ======================================================================
/// 通知の内容に関する設定
///
/// どのタイミングでどのような通知を表示するかを制御します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 成功通知機能全体の有効/無効スイッチ
    #[serde(default)]
    pub enabled: bool,
    /// 実行されたモード名を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_mode: bool,
    /// 加工結果を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_result: bool,
    /// 一時停止の切り替えを通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_pause: bool,
}

impl Default for NotificationSettings {
    /// デフォルトの通知設定を生成する
    ///
    /// # Returns
    /// * `Self` - 通知オフ・各サブ設定はオンのデフォルト設定。
    fn default() -> Self {
        Self {
            enabled: false,
            notify_mode: true,
            notify_result: true,
            notify_pause: true,
        }
    }
}

// ======================================================================
// アプリケーション設定
// ======================================================================
/// アプリケーションの設定情報
///
/// JSONファイルとして保存・読み込みされるアプリケーション全体の構成設定です。
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
    /// 監視が一時停止されているかどうか
    #[serde(default)]
    pub is_paused: bool,
    /// 通知の内容設定
    #[serde(default)]
    pub notification_settings: NotificationSettings,
}

impl Default for AppConfig {
    /// デフォルトのアプリケーション設定を生成する
    ///
    /// # Returns
    /// * `Self` - 標準的な動作環境のためのデフォルト設定。
    fn default() -> Self {
        Self {
            mode: RefineMode::UrlDecode,
            interval_ms: 1000,
            monitor_mode: MonitorMode::default(),
            history_enabled: false,
            is_paused: false,
            notification_settings: NotificationSettings::default(),
        }
    }
}

// ======================================================================
// 設定ファイル操作
// ======================================================================
impl AppConfig {
    /// 設定ファイルの保存先パスをシステムOSに合わせて取得する
    ///
    /// # Returns
    /// * `Result<PathBuf>` - 設定ファイルの完全なパス。
    fn config_path() -> Result<PathBuf> {
        let config_dir = get_config_dir()?;
        fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
        Ok(config_dir.join("config.json"))
    }

    /// 設定ファイルを読み込む
    ///
    /// 存在しない場合や失敗した場合はデフォルト設定を返します。
    ///
    /// # Returns
    /// * `Self` - ファイルから読み込まれた `AppConfig`、またはデフォルトの `AppConfig`。
    pub fn load() -> Self {
        // 設定ファイルパス取得
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                crate::log_warn!("設定ファイルパスの取得に失敗: {:?}", e);
                return Self::default();
            }
        };

        // 設定ファイルが存在しない → デフォルトで継続
        if !config_path.exists() {
            return Self::default();
        }

        // ファイル読み込み
        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("設定ファイルの読み込みに失敗: {:?}", e);
                return Self::default();
            }
        };

        // JSON パース
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(e) => {
                crate::log_warn!("設定ファイルの解析に失敗: {:?}", e);
                Self::default()
            }
        }
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// # Returns
    /// * `Result<()>` - 保存が成功した場合は `Ok(())`、失敗した場合は `Err` を返します。
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path().map_err(|e| {
            crate::log_error!("設定ファイルパスの取得に失敗: {:?}", e);
            e
        })?;

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            crate::log_error!("設定のシリアライズに失敗: {:?}", e);
            e
        })?;

        fs::write(&config_path, content).map_err(|e| {
            crate::log_error!("設定ファイルの書き込みに失敗: {:?}", e);
            e
        })?;

        Ok(())
    }
}

// ======================================================================
// 設定ディレクトリ
// ======================================================================
/// 設定ディレクトリのパスを取得する
///
/// OSに応じたアプリケーション設定ディレクトリのパスを計算します。
///
/// # Returns
/// * `Result<PathBuf>` - OSに応じた設定ディレクトリのパス。
pub fn get_config_dir() -> Result<PathBuf> {
    let base_dirs =
        directories::BaseDirs::new().context("システムディレクトリの取得に失敗しました")?;

    #[cfg(windows)]
    let dir_name = consts::APP_NAME;

    #[cfg(not(windows))]
    let dir_name = consts::APP_NAME_KEBAB;

    Ok(base_dirs.config_dir().join(dir_name))
}

// ======================================================================
// テスト
// ======================================================================
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

    /// NotificationSettings のデフォルト値が正しいこと
    #[test]
    fn test_notification_settings_default() {
        let ns = NotificationSettings::default();
        assert!(!ns.enabled, "enabled のデフォルトは false");
        assert!(ns.notify_mode);
        assert!(ns.notify_result);
        assert!(ns.notify_pause);
    }

    /// 古い設定 JSON (show_success_notification フィールドあり) を読んでも
    /// デフォルト値でデシリアライズできること
    #[test]
    fn test_app_config_backward_compat_old_field() {
        // mode の serde 表現は enum のバリアント名そのまま ("UrlDecode")
        let old_json = r#"{
            "mode": "UrlDecode",
            "interval_ms": 1000,
            "show_success_notification": true
        }"#;
        let config: AppConfig = serde_json::from_str(old_json).unwrap();
        // 未知フィールドは無視され、notification_settings はデフォルト値になる
        assert_eq!(config.interval_ms, 1000);
        assert!(!config.notification_settings.enabled);
    }

    /// notification_settings.enabled が JSON に保存・復元されること
    #[test]
    fn test_notification_settings_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.notification_settings.notify_result = false;

        let json = serde_json::to_string(&config).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert!(decoded.notification_settings.enabled);
        assert!(!decoded.notification_settings.notify_result);
    }
}

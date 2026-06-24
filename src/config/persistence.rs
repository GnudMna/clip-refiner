use std::fs;
use std::path::{Path, PathBuf};

use super::paths::get_config_file_path;
use super::types::AppConfig;

use anyhow::Result;

impl AppConfig {
    /// 設定ファイルの保存先パスをシステムOSに合わせて取得する
    ///
    /// # Returns
    /// * `Result<PathBuf>` - 設定ファイルの完全なパス
    fn config_path() -> Result<PathBuf> {
        get_config_file_path()
    }

    /// 設定ファイルを読み込む
    ///
    /// 存在しない場合や失敗した場合はデフォルト設定を返す
    /// 解析に失敗した場合は元ファイルを `config.json.bak` へ退避する
    ///
    /// # Returns
    /// * `Self` - ファイルから読み込まれた `AppConfig`、またはデフォルトの `AppConfig`
    pub fn load() -> Self {
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                crate::log_warn!("設定ファイルパスの取得に失敗: {:?}", e);
                return Self::default();
            }
        };

        if !config_path.exists() {
            return Self::default();
        }

        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("設定ファイルの読み込みに失敗: {:?}", e);
                return Self::default();
            }
        };

        match serde_json::from_str::<AppConfig>(&content) {
            Ok(mut config) => {
                config.normalize();
                config.hotkeys.fix_invalid();
                config
            }
            Err(e) => {
                crate::log_warn!("設定ファイルの解析に失敗: {:?}", e);
                backup_corrupted_config(&config_path);
                Self::default()
            }
        }
    }

    /// 現在の設定をファイルへ保存する
    ///
    /// # Returns
    /// * `Result<()>` - 保存が成功した場合は `Ok(())`、失敗した場合は `Err` を返す
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path().map_err(|e| {
            crate::log_error!("設定ファイルパスの取得に失敗: {:?}", e);
            e
        })?;

        let mut to_save = self.clone();
        to_save.normalize();

        let content = serde_json::to_string_pretty(&to_save).map_err(|e| {
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

/// 破損した設定ファイルをバックアップする
fn backup_corrupted_config(config_path: &Path) {
    let backup_path = config_path.with_file_name("config.json.bak");
    match fs::copy(config_path, &backup_path) {
        Ok(_) => {
            crate::log_info!(
                "破損した設定をバックアップしました: {}",
                backup_path.display()
            );
        }
        Err(e) => {
            crate::log_warn!("設定ファイルのバックアップに失敗: {:?}", e);
        }
    }
}

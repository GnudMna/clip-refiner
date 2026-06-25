use std::fs;
use std::path::{Path, PathBuf};

use super::paths::get_config_file_path;
use super::permissions::restrict_private_file_permissions;
use super::serialize::config_to_toml;
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
    /// 存在しない場合はデフォルト設定を生成し、説明コメント付きの `config.toml` を保存する
    /// 読み込みや解析に失敗した場合はデフォルト設定を返す
    /// 解析に失敗した場合は元ファイルを `config.toml.bak` へ退避する
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
            return Self::create_initial_config(&config_path);
        }

        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("設定ファイルの読み込みに失敗: {:?}", e);
                return Self::default();
            }
        };

        match toml::from_str::<AppConfig>(&content) {
            Ok(config) => {
                let (config, migrated) = config.prepare_loaded();
                if migrated && let Err(e) = config.save() {
                    crate::log_warn!("移行後の設定保存に失敗: {:?}", e);
                }
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

        let existing = if config_path.exists() {
            fs::read_to_string(&config_path).ok()
        } else {
            None
        };

        let content = config_to_toml(&to_save, existing.as_deref()).map_err(|e| {
            crate::log_error!("設定のシリアライズに失敗: {:?}", e);
            e
        })?;

        fs::write(&config_path, content).map_err(|e| {
            crate::log_error!("設定ファイルの書き込みに失敗: {:?}", e);
            e
        })?;

        if let Err(e) = restrict_private_file_permissions(&config_path) {
            crate::log_warn!("設定ファイルのパーミッション設定に失敗: {:?}", e);
        }

        Ok(())
    }

    /// 初回起動用のデフォルト設定を生成し、説明コメント付きで保存する
    ///
    /// 保存に失敗してもデフォルト設定は返す
    fn create_initial_config(config_path: &Path) -> Self {
        let config = Self::default();
        match config.save() {
            Ok(()) => {
                crate::log_info!("初回設定ファイルを作成しました: {}", config_path.display());
            }
            Err(e) => {
                crate::log_warn!("初回設定ファイルの作成に失敗: {:?}", e);
            }
        }
        config
    }
}

/// 破損した設定ファイルをバックアップする
fn backup_corrupted_config(config_path: &Path) {
    let backup_path = config_path.with_file_name("config.toml.bak");
    match fs::copy(config_path, &backup_path) {
        Ok(_) => {
            if let Err(e) = restrict_private_file_permissions(&backup_path) {
                crate::log_warn!("設定バックアップのパーミッション設定に失敗: {:?}", e);
            }
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

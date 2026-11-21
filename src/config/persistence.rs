use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::paths::{config_file_modified_time, get_config_file_path};
use super::permissions::restrict_private_file_permissions;
use super::registered_clips::{
    load_registered_clips, migrate_legacy_clip_images, save_registered_clips,
};
use super::serialize::config_to_toml;
use super::types::{AppConfig, RegisteredClip};

use anyhow::Result;

// ======================================================================
// 設定の永続化
// ======================================================================
/// 設定ファイルの再読み込みエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigReloadError {
    /// 設定ファイルパスの取得に失敗
    Path(String),
    /// 設定ファイルが存在しない
    NotFound,
    /// ファイル読み取りに失敗
    Read(String),
    /// TOML 解析に失敗
    Parse(String),
}

impl ConfigReloadError {
    /// ユーザー向けのエラーメッセージを返す
    pub fn user_message(&self) -> &str {
        match self {
            Self::Path(_) => "設定ファイルの場所を取得できませんでした",
            Self::NotFound => "設定ファイルが見つかりません",
            Self::Read(_) => "設定ファイルを読み取れませんでした",
            Self::Parse(_) => "設定ファイルの形式が不正です",
        }
    }
}

/// ディスク上の設定ファイルの最終更新時刻を取得する
///
/// # Returns
/// * `Result<Option<SystemTime>>` - 更新時刻。取得失敗時は `Err`
pub fn disk_config_modified_time() -> Result<Option<SystemTime>> {
    config_file_modified_time()
}

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

        match parse_config_toml(&content) {
            Ok((config, legacy_clips)) => {
                let (config, migrated) = finish_loading(config, legacy_clips);
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

    /// ディスク上の設定ファイルを再読み込みする
    ///
    /// スキーマ移行が必要な場合は移行後の内容を保存する
    ///
    /// # Returns
    /// * `Result<(Self, bool), ConfigReloadError>` - 読み込んだ設定と、移行が実行されたかどうか
    pub fn reload_from_disk() -> Result<(Self, bool), ConfigReloadError> {
        let config_path =
            Self::config_path().map_err(|e| ConfigReloadError::Path(e.to_string()))?;

        if !config_path.exists() {
            return Err(ConfigReloadError::NotFound);
        }

        let content =
            fs::read_to_string(&config_path).map_err(|e| ConfigReloadError::Read(e.to_string()))?;

        let (config, legacy_clips) =
            parse_config_toml(&content).map_err(|e| ConfigReloadError::Parse(e.to_string()))?;

        let (config, migrated) = finish_loading(config, legacy_clips);
        if migrated && let Err(e) = config.save() {
            crate::log_warn!("移行後の設定保存に失敗: {:?}", e);
        }

        Ok((config, migrated))
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

        save_registered_clips(&to_save.clips).map_err(|e| {
            crate::log_error!("登録クリップの保存に失敗: {:?}", e);
            e
        })?;

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

// ======================================================================
// プライベート関数
// ======================================================================
/// `config.toml` を解析し、レガシー `[[clips]]` を分離して返す
fn parse_config_toml(content: &str) -> Result<(AppConfig, Vec<RegisteredClip>)> {
    let value: toml::Value = toml::from_str(content)?;
    let legacy_clips = value
        .get("clips")
        .and_then(|clips| clips.clone().try_into().ok())
        .unwrap_or_default();
    let config: AppConfig = value.try_into()?;
    Ok((config, legacy_clips))
}

/// スキーマ移行・登録クリップ読み込み・レガシー移行を行う
fn finish_loading(config: AppConfig, legacy_clips: Vec<RegisteredClip>) -> (AppConfig, bool) {
    let (mut config, schema_migrated) = config.prepare_loaded();

    let file_clips = load_registered_clips().unwrap_or_else(|err| {
        crate::log_warn!("登録クリップファイルの読み込みに失敗: {:?}", err);
        Vec::new()
    });

    let mut clips_migrated = false;
    if !file_clips.is_empty() {
        config.clips = file_clips;
    } else if !legacy_clips.is_empty() {
        config.clips = legacy_clips;
        clips_migrated = true;
        crate::log_info!("config.toml の [[clips]] を registered-clips.dat へ移行する");
    }

    if migrate_legacy_clip_images(&mut config.clips) {
        clips_migrated = true;
    }

    config.normalize_clips();

    (config, schema_migrated || clips_migrated)
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `config.toml` の `[[clips]]` は別ファイルへ移行され、TOML から除去されること
    #[test]
    fn legacy_clips_in_config_toml_are_migrated_to_separate_file() {
        crate::test_helpers::with_temp_config_dir(|| {
            let toml_str = r#"
version = 2
mode = "Trim"
interval_ms = 1000

[[clips]]
label = "secret-label"
text = "secret-body"
"#;
            let (config, legacy_clips) = parse_config_toml(toml_str).expect("parse");
            assert_eq!(legacy_clips.len(), 1);
            assert!(config.clips.is_empty());

            let (config, migrated) = finish_loading(config, legacy_clips);
            assert!(migrated);
            assert_eq!(config.clips[0].text, "secret-body");

            config.save().expect("save");
            let content =
                fs::read_to_string(AppConfig::config_path().expect("path")).expect("read");
            assert!(!content.contains("[[clips]]"));
            assert!(!content.contains("secret-body"));

            let file_clips = load_registered_clips().expect("load clips file");
            assert_eq!(file_clips.len(), 1);
            assert_eq!(file_clips[0].text, "secret-body");
        });
    }
}

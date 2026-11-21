//! 設定ファイルの保存と更新時刻・内容指紋の取得

use std::fs;
use std::time::SystemTime;

use super::super::atomic_io::write_text_atomically;
use super::super::paths::{config_file_modified_time, get_config_file_path};
use super::super::permissions::restrict_private_file_permissions;
use super::super::registered_clips::save_registered_clips;
use super::super::serialize::config_to_toml;
use super::super::types::AppConfig;
use crate::error::ConfigError;

// ======================================================================
// 内容指紋
// ======================================================================
/// `config.toml` 本文の内容指紋 (BLAKE3)
///
/// 更新時刻ではなくバイト列で比較し、同一秒の上書きや時計ずれでも変更を検知する
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigFileFingerprint([u8; 32]);

/// ディスク上の設定ファイルの最終更新時刻を取得する
///
/// # Returns
/// * `Result<Option<SystemTime>>` - 更新時刻。取得失敗時は `Err`
///
/// # Errors
/// 設定ファイルのメタデータ取得に失敗した場合
pub fn disk_config_modified_time() -> Result<Option<SystemTime>, ConfigError> {
    config_file_modified_time()
}

/// ディスク上の設定ファイル本文の内容指紋を取得する
///
/// ファイルが無い場合は `None` を返す
///
/// # Returns
/// * `Result<Option<ConfigFileFingerprint>, ConfigError>` - 指紋。未作成時は `None`
///
/// # Errors
/// 設定ファイルパス取得または読み取りに失敗した場合
pub fn disk_config_content_fingerprint() -> Result<Option<ConfigFileFingerprint>, ConfigError> {
    let path = get_config_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(ConfigError::Read)?;
    Ok(Some(ConfigFileFingerprint(
        *blake3::hash(&bytes).as_bytes(),
    )))
}

impl AppConfig {
    /// 現在の設定をファイルへ保存する
    ///
    /// # Arguments
    /// * `self` - 保存対象の設定
    ///
    /// # Returns
    /// * `Result<()>` - 保存に成功した場合は `Ok(())`
    ///
    /// # Errors
    /// 設定パス取得、登録クリップ保存、TOML 書き出し、パーミッション設定のいずれかに失敗した場合
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = get_config_file_path().map_err(|e| {
            tracing::error!("設定ファイルパスの取得に失敗: {}", e);
            e
        })?;

        let mut to_save = self.clone();
        to_save.normalize();

        save_registered_clips(&to_save.clips).map_err(|e| {
            tracing::error!("登録クリップの保存に失敗: {:?}", e);
            ConfigError::RegisteredClipsSave(e.to_string())
        })?;

        let existing = if config_path.exists() {
            fs::read_to_string(&config_path).ok()
        } else {
            None
        };

        let content = config_to_toml(&to_save, existing.as_deref()).map_err(|e| {
            tracing::error!("設定のシリアライズに失敗: {:?}", e);
            ConfigError::Serialize(e.to_string())
        })?;

        // 一時ファイル経由で置き換え、途中書き込みによる破損を防ぐ
        write_text_atomically(&config_path, &content).map_err(|e| {
            tracing::error!("設定ファイルの書き込みに失敗: {:?}", e);
            ConfigError::Write(e)
        })?;

        if let Err(e) = restrict_private_file_permissions(&config_path) {
            tracing::warn!("設定ファイルのパーミッション設定に失敗: {:?}", e);
        }

        Ok(())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::paths::get_config_file_path;

    /// `save` が既存設定を原子的に置き換え、一時ファイルを残さないようにすること
    #[test]
    fn save_replaces_config_atomically_without_temp_files() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            let mut config = AppConfig {
                interval_ms: 1500,
                ..AppConfig::default()
            };
            config.save().expect("初回保存");
            assert!(path.is_file());

            config.interval_ms = 2500;
            config.save().expect("2 回目の保存");

            let content = fs::read_to_string(&path).expect("保存後の読み取り");
            assert!(
                content.contains("2500"),
                "更新後の interval_ms が書き込まれていること: {content}"
            );
            assert!(!path.with_file_name("config.toml.tmp").exists());
            assert!(!path.with_file_name("config.toml.prev").exists());
        });
    }

    /// 未作成の設定ファイルは内容指紋 `None` を返すようにすること
    #[test]
    fn content_fingerprint_none_when_missing() {
        crate::test_helpers::with_temp_config_dir(|| {
            assert_eq!(
                disk_config_content_fingerprint().expect("fingerprint"),
                None
            );
        });
    }

    /// 内容が変わると指紋も変わるようにすること
    #[test]
    fn content_fingerprint_changes_when_bytes_change() {
        crate::test_helpers::with_temp_config_dir(|| {
            let mut config = AppConfig::default();
            config.save().expect("初回保存");
            let first = disk_config_content_fingerprint()
                .expect("fingerprint")
                .expect("存在する");

            config.interval_ms = 2500;
            config.save().expect("内容変更");
            let second = disk_config_content_fingerprint()
                .expect("fingerprint")
                .expect("存在する");

            assert_ne!(first, second);
        });
    }

    /// 同一バイト列の再書き込みでは指紋が変わらないようにすること
    #[test]
    fn content_fingerprint_stable_for_identical_bytes() {
        crate::test_helpers::with_temp_config_dir(|| {
            AppConfig::default().save().expect("初回保存");
            let path = get_config_file_path().expect("path");
            let bytes = fs::read(&path).expect("read");
            let first = disk_config_content_fingerprint()
                .expect("fingerprint")
                .expect("存在する");

            fs::write(&path, &bytes).expect("同一バイト列");
            let second = disk_config_content_fingerprint()
                .expect("fingerprint")
                .expect("存在する");

            assert_eq!(first, second);
        });
    }
}

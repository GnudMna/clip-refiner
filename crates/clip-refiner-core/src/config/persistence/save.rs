//! 設定ファイルの保存と更新時刻・内容指紋の取得

use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::atomic_io::write_text_atomically;
use super::super::paths::{config_file_modified_time, get_config_file_path};
use super::super::permissions::restrict_private_file_permissions;
use super::super::registered_clips::save_registered_clips;
use super::super::serialize::config_to_toml;
use super::super::types::AppConfig;
use crate::error::ConfigError;

// ======================================================================
// 定数
// ======================================================================
/// 保存前に既存の設定ファイルを読む最大試行回数
const EXISTING_READ_ATTEMPTS: u32 = 4;
/// 既存の設定ファイルの読み取り再試行までの待ち時間
const EXISTING_READ_RETRY_INTERVAL: Duration = Duration::from_millis(100);

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
/// 設定ファイルパス取得、メタデータ取得、更新時刻取得のいずれかに失敗した場合
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
    /// 設定パス取得、既存ファイル読み取り、登録クリップ保存、TOML 生成、書き込みのいずれかに失敗した場合
    /// (パーミッション設定の失敗は警告のみ)
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = get_config_file_path().map_err(|e| {
            tracing::error!("設定ファイルパスの取得に失敗: {}", e);
            e
        })?;

        let mut to_save = self.clone();
        to_save.normalize();

        // 登録クリップの保存より前に読む (読めない場合に一部だけ書き込まれた状態を避ける)
        let existing = read_existing_config(&config_path)?;

        save_registered_clips(&to_save.clips).map_err(|e| {
            tracing::error!("登録クリップの保存に失敗: {:?}", e);
            ConfigError::RegisteredClipsSave(e.to_string())
        })?;

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
// プライベート関数
// ======================================================================
/// コメントや未知のキーを引き継ぐため、保存前に既存の設定ファイルを読む
///
/// 一時的なロック (エディタやウイルス対策ソフトなど) に備えて短くリトライする
/// リトライしても読めない場合は、コメントや未知のキーを黙って消さないよう保存を中止する
/// UTF-8 として不正な場合は引き継げる内容がないため、警告を出して `None` を返す
///
/// # Arguments
/// * `path` - 設定ファイルのパス
///
/// # Returns
/// * `Result<Option<String>, ConfigError>` - 既存の本文。ファイルが無い場合や UTF-8 として不正な場合は `None`
///
/// # Errors
/// リトライしても既存ファイルを読み取れなかった場合
fn read_existing_config(path: &Path) -> Result<Option<String>, ConfigError> {
    for attempt in 1..=EXISTING_READ_ATTEMPTS {
        match fs::read_to_string(path) {
            Ok(text) => return Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                tracing::warn!(
                    "既存の設定ファイルが UTF-8 ではないため、コメントや未知のキーを引き継がずに保存する: {:?}",
                    e
                );
                return Ok(None);
            }
            Err(e) if attempt == EXISTING_READ_ATTEMPTS => {
                tracing::error!(
                    "既存の設定ファイルを読めないため保存を中止する ({} 回試行): {:?}",
                    attempt,
                    e
                );
                return Err(ConfigError::Read(e));
            }
            Err(e) => {
                tracing::warn!(
                    "既存の設定ファイルの読み込みに失敗したため再試行する ({}/{}): {:?}",
                    attempt,
                    EXISTING_READ_ATTEMPTS,
                    e
                );
                thread::sleep(EXISTING_READ_RETRY_INTERVAL);
            }
        }
    }
    Ok(None)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::paths::get_config_file_path;
    use crate::config::registered_clips::{load_registered_clips, registered_clip_from_plain_text};

    /// 既存ファイルを読めない場合は保存を中止し、登録クリップも書き込まないこと
    ///
    /// 設定ファイルの位置にディレクトリを置いて読み取り失敗を再現する
    #[test]
    fn save_aborts_when_existing_config_is_unreadable() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            fs::create_dir_all(&path).expect("読めないエントリ");

            let config = AppConfig {
                clips: vec![
                    registered_clip_from_plain_text("ラベル".to_string(), "本文")
                        .expect("登録クリップ"),
                ],
                ..AppConfig::default()
            };
            let result = config.save();

            assert!(
                matches!(result, Err(ConfigError::Read(_))),
                "読み取り失敗で中止されること: {result:?}"
            );
            assert!(path.is_dir(), "既存エントリが置き換えられないこと");
            // 登録クリップのストアにも書き込まれていない
            let (clips, _) = load_registered_clips().expect("登録クリップの読み込み");
            assert!(clips.is_empty(), "登録クリップが書き込まれないこと");
        });
    }

    /// 既存ファイルが UTF-8 でなくても保存できること
    #[test]
    fn save_succeeds_when_existing_config_is_not_utf8() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            fs::write(&path, [0xff, 0xfe, 0x00]).expect("不正なバイト列");

            AppConfig::default().save().expect("保存");

            assert!(fs::read_to_string(&path).is_ok());
        });
    }

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
            let dir = path.parent().expect("親ディレクトリ");
            for entry in fs::read_dir(dir).expect("ディレクトリ読み取り") {
                let name = entry.expect("エントリ").file_name();
                let name = name.to_string_lossy();
                assert!(
                    !name.ends_with(".tmp") && !name.ends_with(".prev"),
                    "一時ファイルが残っている: {name}"
                );
            }
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

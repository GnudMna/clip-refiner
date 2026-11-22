//! 設定ファイルの保存と更新時刻・内容指紋の取得

use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::atomic_io::write_text_atomically;
use super::super::paths::{config_file_modified_time, get_config_file_path};
use super::super::permissions::restrict_private_file_permissions;
use super::super::registered_clips::{RegisteredClipsSaveOutcome, save_registered_clips};
use super::super::serialize::config_to_toml;
use super::super::types::AppConfig;
use super::load::{backup_corrupted_config, parse_config_toml};
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
        self.save_with_outcome().map(|_| ())
    }

    /// 現在の設定をファイルへ保存し、登録クリップの保存結果も返す
    ///
    /// 登録クリップの鍵を取得できない場合は登録クリップの保存だけを省略し、`config.toml` は保存する
    ///
    /// # Arguments
    /// * `self` - 保存対象の設定
    ///
    /// # Returns
    /// * `Result<RegisteredClipsSaveOutcome, ConfigError>` - 設定の保存に成功した場合は登録クリップの保存結果
    ///
    /// # Errors
    /// [`AppConfig::save`] と同様
    pub fn save_with_outcome(&self) -> Result<RegisteredClipsSaveOutcome, ConfigError> {
        self.save_with_fingerprint().map(|(outcome, _)| outcome)
    }

    /// 現在の設定をファイルへ保存し、登録クリップの保存結果と書き込んだ本文の指紋を返す
    ///
    /// 保存後にディスクを読み直すと、保存直後の外部編集を同期済みとして記録してしまう
    /// 書き込んだ本文そのものの指紋を使えば、その編集を次の変更検知で拾える
    ///
    /// # Arguments
    /// * `self` - 保存対象の設定
    ///
    /// # Returns
    /// * `Result<(RegisteredClipsSaveOutcome, ConfigFileFingerprint), ConfigError>` - 登録クリップの保存結果と書き込んだ本文の指紋
    ///
    /// # Errors
    /// [`AppConfig::save`] と同様
    pub fn save_with_fingerprint(
        &self,
    ) -> Result<(RegisteredClipsSaveOutcome, ConfigFileFingerprint), ConfigError> {
        let config_path = get_config_file_path().map_err(|e| {
            tracing::error!("設定ファイルパスの取得に失敗: {}", e);
            e
        })?;

        let mut to_save = self.clone();
        to_save.normalize();

        // 登録クリップの保存より前に読む (読めない場合に一部だけ書き込まれた状態を避ける)
        let existing = read_existing_config(&config_path)?;
        backup_unparsable_config(&config_path, existing.as_deref())?;

        let clips_outcome = save_registered_clips(&to_save.clips).inspect_err(|e| {
            tracing::error!("登録クリップの保存に失敗: {:?}", e);
        })?;

        let content = config_to_toml(&to_save, existing.as_deref()).inspect_err(|e| {
            tracing::error!("設定のシリアライズに失敗: {:?}", e);
        })?;

        // 一時ファイル経由で置き換え、途中書き込みによる破損を防ぐ
        write_text_atomically(&config_path, &content).map_err(|e| {
            tracing::error!("設定ファイルの書き込みに失敗: {:?}", e);
            ConfigError::Write(e)
        })?;

        if let Err(e) = restrict_private_file_permissions(&config_path) {
            tracing::warn!("設定ファイルのパーミッション設定に失敗: {:?}", e);
        }

        Ok((
            clips_outcome,
            ConfigFileFingerprint(*blake3::hash(content.as_bytes()).as_bytes()),
        ))
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

/// 解析できない既存の設定ファイルを、上書きされる前に `config.toml.bak` へ退避する
///
/// 外部編集の途中で壊れたファイルを、アプリ側の保存で既定の内容に置き換えて失わないための処理
/// 退避に失敗した場合は、壊れた内容を失わないよう保存を中止する
///
/// # Arguments
/// * `path` - 設定ファイルのパス
/// * `existing` - 保存前に読んだ本文。ファイルが無い場合や UTF-8 として不正な場合は `None`
///
/// # Errors
/// 解析できない既存ファイルの退避に失敗した場合
fn backup_unparsable_config(path: &Path, existing: Option<&str>) -> Result<(), ConfigError> {
    let unparsable = match existing {
        Some(text) => parse_config_toml(text).is_err(),
        // UTF-8 として不正なファイルは `None` になるため、ファイルの有無で見分ける
        None => path.is_file(),
    };
    if unparsable && !backup_corrupted_config(path) {
        tracing::error!("解析できない設定ファイルを退避できないため保存を中止する");
        return Err(ConfigError::Write(io::Error::other(
            "解析できない設定ファイルを config.toml.bak へ退避できませんでした",
        )));
    }
    Ok(())
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

    /// 登録クリップの鍵を復号できなくても、`config.toml` を保存できること
    ///
    /// 読み込めなかった既存の `registered-clips.dat` は、空の一覧でも削除されない
    #[test]
    fn save_succeeds_when_clip_store_key_is_undecryptable() {
        crate::test_helpers::with_temp_config_dir(|| {
            let config_path = get_config_file_path().expect("path");
            let dir = config_path.parent().expect("親ディレクトリ");
            fs::write(dir.join("registered-clips.key"), b"broken-key").expect("不正な鍵");
            fs::write(dir.join("registered-clips.dat"), b"existing").expect("既存のデータ");
            crate::config::clip_store_key::clear_clip_store_key_cache();
            // 起動時の読み込み失敗を再現する (既存ファイルを未読込として保護する)
            assert!(load_registered_clips().is_err());

            let config = AppConfig {
                interval_ms: 2500,
                ..AppConfig::default()
            };
            config.save().expect("鍵を読めなくても保存できること");

            let content = fs::read_to_string(&config_path).expect("config.toml");
            assert!(
                content.contains("2500"),
                "設定が書き込まれること: {content}"
            );
            assert_eq!(
                fs::read(dir.join("registered-clips.dat")).expect("dat"),
                b"existing",
                "既存の登録クリップファイルが変更されないこと"
            );
        });
    }

    /// 鍵を復号できず登録クリップの保存を省略した場合は、その結果を返すこと
    #[test]
    fn save_with_outcome_reports_skipped_clips_when_key_is_undecryptable() {
        crate::test_helpers::with_temp_config_dir(|| {
            let config_path = get_config_file_path().expect("path");
            let config = AppConfig {
                clips: vec![
                    registered_clip_from_plain_text("ラベル".to_string(), "本文")
                        .expect("登録クリップ"),
                ],
                ..AppConfig::default()
            };
            let dir = config_path.parent().expect("親ディレクトリ");
            fs::write(dir.join("registered-clips.key"), b"broken-key").expect("不正な鍵");
            crate::config::clip_store_key::clear_clip_store_key_cache();

            let outcome = config.save_with_outcome().expect("保存");

            assert_eq!(outcome, RegisteredClipsSaveOutcome::SkippedKeyUnavailable);
            assert!(config_path.is_file(), "config.toml は保存されること");
            assert_eq!(
                AppConfig::default().save_with_outcome().expect("空の保存"),
                RegisteredClipsSaveOutcome::Saved,
                "空の一覧では鍵を使わず保存扱いとなること"
            );
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

    /// 構文エラーのある既存ファイルは、上書き前に `config.toml.bak` へ退避すること
    #[test]
    fn save_backs_up_unparsable_config_before_overwriting() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            fs::write(&path, "mode = [[invalid").expect("壊れた設定");

            AppConfig::default().save().expect("保存");

            assert_eq!(
                fs::read(path.with_file_name("config.toml.bak")).expect("backup"),
                b"mode = [[invalid"
            );
            assert!(
                parse_config_toml(&fs::read_to_string(&path).expect("read")).is_ok(),
                "config.toml は正常な内容へ置き換わること"
            );
        });
    }

    /// 解析できる既存ファイルは退避しないこと
    #[test]
    fn save_does_not_back_up_parsable_config() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            AppConfig::default().save().expect("初回保存");
            AppConfig::default().save().expect("2 回目の保存");

            assert!(!path.with_file_name("config.toml.bak").exists());
        });
    }

    /// 退避できない場合は、壊れた設定ファイルを上書きせず保存を中止すること
    ///
    /// `.bak` の位置にディレクトリを置いてコピー失敗を再現する
    #[test]
    fn save_aborts_when_backup_of_unparsable_config_fails() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            fs::write(&path, "mode = [[invalid").expect("壊れた設定");
            fs::create_dir_all(path.with_file_name("config.toml.bak")).expect("退避先を塞ぐ");

            let result = AppConfig::default().save();

            assert!(
                matches!(result, Err(ConfigError::Write(_))),
                "退避失敗で中止されること: {result:?}"
            );
            assert_eq!(
                fs::read(&path).expect("read"),
                b"mode = [[invalid",
                "壊れた設定が上書きされないこと"
            );
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

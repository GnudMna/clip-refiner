use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::atomic_io::write_text_atomically;
use super::paths::{config_file_modified_time, get_config_file_path};
use super::permissions::restrict_private_file_permissions;
use super::registered_clips::{
    load_registered_clips, migrate_legacy_clip_images, registered_clip_from_plain_text,
    save_registered_clips,
};
use super::serialize::config_to_toml;
use super::types::{AppConfig, RegisteredClip};
use super::unknown_keys::{collect_unknown_config_keys, warn_unknown_config_keys};

use crate::error::ConfigError;

use serde::Deserialize;

// ======================================================================
// 設定の永続化
// ======================================================================
/// 設定ファイルの再読み込みエラー
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigReloadError {
    /// 設定ファイルパスの取得に失敗
    #[error("設定ファイルの場所を取得できませんでした: {0}")]
    Path(String),
    /// 設定ファイルが存在しない
    #[error("設定ファイルが見つかりません")]
    NotFound,
    /// ファイル読み取りに失敗
    #[error("設定ファイルを読み取れませんでした: {0}")]
    Read(String),
    /// TOML 解析に失敗
    #[error("設定ファイルの形式が不正です: {0}")]
    Parse(String),
}

impl ConfigReloadError {
    /// ユーザー向けのエラーメッセージを返す
    ///
    /// # Arguments
    /// * `self` - 再読み込みエラー
    ///
    /// # Returns
    /// * `&str` - UI 表示用の固定メッセージ
    #[must_use]
    pub fn user_message(&self) -> &str {
        match self {
            Self::Path(_) => "設定ファイルの場所を取得できませんでした",
            Self::NotFound => "設定ファイルが見つかりません",
            Self::Read(_) => "設定ファイルを読み取れませんでした",
            Self::Parse(_) => "設定ファイルの形式が不正です",
        }
    }
}

/// 設定読み込みの結果
#[derive(Debug, Clone)]
pub struct ConfigLoadOutcome {
    /// 読み込んだ設定 (失敗時はデフォルト)
    pub config: AppConfig,
    /// デフォルトへフォールバックした場合の理由
    pub fallback: Option<ConfigLoadFallback>,
    /// 認識できない設定キー (解析成功時のみ。読み込み自体は継続)
    pub unknown_keys: Vec<String>,
}

/// ディスクからの設定再読み込み結果
#[derive(Debug, Clone)]
pub struct ConfigDiskReload {
    /// 読み込んだ設定
    pub config: AppConfig,
    /// スキーマ移行またはレガシー移行を実行したか
    pub migrated: bool,
    /// 将来の `version` を現行へ揃えたか
    pub newer_version_clamped: bool,
    /// 認識できない設定キー (読み込み自体は継続)
    pub unknown_keys: Vec<String>,
}

/// 設定読み込み失敗時にデフォルトへフォールバックした理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoadFallback {
    /// 失敗理由
    pub error: ConfigReloadError,
    /// 解析失敗時に `config.toml.bak` を作成できたか
    pub backup_created: bool,
}

impl ConfigLoadFallback {
    /// デスクトップ通知・CLI 向けのメッセージを返す
    ///
    /// # Arguments
    /// * `self` - フォールバック理由
    ///
    /// # Returns
    /// * `String` - エラー内容とバックアップ成否を含むメッセージ
    #[must_use]
    pub fn user_message(&self) -> String {
        let core = match &self.error {
            ConfigReloadError::Parse(detail) => {
                format!("{} ({detail})", self.error.user_message())
            }
            _ => self.error.user_message().to_string(),
        };
        let suffix = match (&self.error, self.backup_created) {
            (ConfigReloadError::Parse(_), true) => {
                "。デフォルト設定で起動します。元ファイルは config.toml.bak に保存しました"
            }
            (ConfigReloadError::Parse(_), false) => {
                "。デフォルト設定で起動します。元ファイルのバックアップに失敗しました"
            }
            _ => "。デフォルト設定で起動します",
        };
        format!("{core}{suffix}")
    }
}

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

impl AppConfig {
    /// 設定ファイルの保存先パスをシステムOSに合わせて取得する
    ///
    /// # Returns
    /// * `Result<PathBuf>` - 設定ファイルの完全なパス
    fn config_path() -> Result<PathBuf, ConfigError> {
        get_config_file_path()
    }

    /// 設定ファイルを読み込む
    ///
    /// 存在しない場合はデフォルト設定を生成し、説明コメント付きの `config.toml` を保存する
    /// 読み込みや解析に失敗した場合はデフォルト設定を返し、[`ConfigLoadOutcome::fallback`] に理由を格納する
    /// 解析に失敗した場合は元ファイルを `config.toml.bak` へ退避する
    /// 認識できないキーは [`ConfigLoadOutcome::unknown_keys`] に格納し、読み込み自体は継続する
    ///
    /// # Returns
    /// * `ConfigLoadOutcome` - 読み込んだ設定と、フォールバック時の理由・未知キー
    pub fn load() -> ConfigLoadOutcome {
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(e) => {
                let fallback = ConfigLoadFallback {
                    error: ConfigReloadError::Path(e.to_string()),
                    backup_created: false,
                };
                tracing::error!("{}", fallback.user_message());
                return ConfigLoadOutcome {
                    config: Self::default(),
                    fallback: Some(fallback),
                    unknown_keys: Vec::new(),
                };
            }
        };

        if !config_path.exists() {
            return ConfigLoadOutcome {
                config: Self::create_initial_config(&config_path),
                fallback: None,
                unknown_keys: Vec::new(),
            };
        }

        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                let fallback = ConfigLoadFallback {
                    error: ConfigReloadError::Read(e.to_string()),
                    backup_created: false,
                };
                tracing::error!("{}", fallback.user_message());
                return ConfigLoadOutcome {
                    config: Self::default(),
                    fallback: Some(fallback),
                    unknown_keys: Vec::new(),
                };
            }
        };

        match parse_config_toml(&content) {
            Ok(parsed) => {
                warn_unknown_config_keys(&parsed.unknown_keys);
                let (config, migrated, newer_version_clamped) =
                    finish_loading(parsed.config, parsed.legacy_clips);
                if newer_version_clamped {
                    notify_newer_config_version_clamped();
                }
                if migrated && let Err(e) = config.save() {
                    tracing::warn!("移行後の設定保存に失敗: {:?}", e);
                }
                ConfigLoadOutcome {
                    config,
                    fallback: None,
                    unknown_keys: parsed.unknown_keys,
                }
            }
            Err(e) => {
                let backup_created = backup_corrupted_config(&config_path);
                let fallback = ConfigLoadFallback {
                    error: ConfigReloadError::Parse(e.to_string()),
                    backup_created,
                };
                tracing::error!("{}", fallback.user_message());
                ConfigLoadOutcome {
                    config: Self::default(),
                    fallback: Some(fallback),
                    unknown_keys: Vec::new(),
                }
            }
        }
    }

    /// ディスク上の設定ファイルを再読み込みする
    ///
    /// スキーマ移行が必要な場合は移行後の内容を保存する
    ///
    /// # Returns
    /// * `Result<ConfigDiskReload, ConfigReloadError>` - 読み込んだ設定と移行・警告メタデータ
    ///
    /// # Errors
    /// 設定ファイルが存在しない、読み取り失敗、TOML 解析失敗のいずれか
    pub fn reload_from_disk() -> Result<ConfigDiskReload, ConfigReloadError> {
        let config_path =
            Self::config_path().map_err(|e| ConfigReloadError::Path(e.to_string()))?;

        if !config_path.exists() {
            return Err(ConfigReloadError::NotFound);
        }

        let content =
            fs::read_to_string(&config_path).map_err(|e| ConfigReloadError::Read(e.to_string()))?;

        let parsed =
            parse_config_toml(&content).map_err(|e| ConfigReloadError::Parse(e.to_string()))?;
        warn_unknown_config_keys(&parsed.unknown_keys);

        let (config, migrated, newer_version_clamped) =
            finish_loading(parsed.config, parsed.legacy_clips);
        if newer_version_clamped {
            notify_newer_config_version_clamped();
        }
        if migrated && let Err(e) = config.save() {
            tracing::warn!("移行後の設定保存に失敗: {:?}", e);
        }

        Ok(ConfigDiskReload {
            config,
            migrated,
            newer_version_clamped,
            unknown_keys: parsed.unknown_keys,
        })
    }

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
        let config_path = Self::config_path().map_err(|e| {
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

    /// 初回起動用のデフォルト設定を生成し、説明コメント付きで保存する
    ///
    /// 保存に失敗してもデフォルト設定は返す
    fn create_initial_config(config_path: &Path) -> Self {
        let config = Self::default();
        match config.save() {
            Ok(()) => {
                tracing::info!("初回設定ファイルを作成しました: {}", config_path.display());
            }
            Err(e) => {
                tracing::warn!("初回設定ファイルの作成に失敗: {:?}", e);
            }
        }
        config
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `config.toml` のレガシー `[[clips]]` (平文 `text`)
#[derive(Debug, Deserialize)]
struct LegacyTomlRegisteredClip {
    #[serde(default)]
    label: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    image_file: Option<String>,
}

/// `config.toml` の解析結果
struct ParsedConfigToml {
    config: AppConfig,
    legacy_clips: Vec<LegacyTomlRegisteredClip>,
    unknown_keys: Vec<String>,
}

/// `config.toml` を解析し、レガシー `[[clips]]` と未知キーを分離して返す
fn parse_config_toml(content: &str) -> anyhow::Result<ParsedConfigToml> {
    let value: toml::Value = toml::from_str(content)?;
    let unknown_keys = collect_unknown_config_keys(&value);
    let legacy_clips = value
        .get("clips")
        .and_then(|clips| clips.clone().try_into().ok())
        .unwrap_or_default();
    let config: AppConfig = value.try_into()?;
    Ok(ParsedConfigToml {
        config,
        legacy_clips,
        unknown_keys,
    })
}

/// レガシー TOML の平文クリップを暗号文保持の `RegisteredClip` へ変換する
fn encrypt_legacy_toml_clips(legacy_clips: Vec<LegacyTomlRegisteredClip>) -> Vec<RegisteredClip> {
    let mut clips = Vec::with_capacity(legacy_clips.len());
    for legacy in legacy_clips {
        if let Some(image_file) = legacy.image_file {
            clips.push(RegisteredClip {
                label: legacy.label,
                preview: String::new(),
                text_ciphertext: Vec::new(),
                image_file: Some(image_file),
            });
            continue;
        }
        match registered_clip_from_plain_text(legacy.label, &legacy.text) {
            Ok(clip) => clips.push(clip),
            Err(err) => {
                tracing::warn!("レガシー登録クリップの暗号化移行をスキップ: {:?}", err);
            }
        }
    }
    clips
}

/// スキーマ移行・登録クリップ読み込み・レガシー移行を行う
fn finish_loading(
    config: AppConfig,
    legacy_clips: Vec<LegacyTomlRegisteredClip>,
) -> (AppConfig, bool, bool) {
    let migration = config.prepare_loaded();
    let mut config = migration.config;
    let schema_migrated = migration.migrated;
    let newer_version_clamped = migration.newer_version_clamped;

    let (file_clips, file_format_migrated) = load_registered_clips().unwrap_or_else(|err| {
        tracing::warn!("登録クリップファイルの読み込みに失敗: {:?}", err);
        (Vec::new(), false)
    });

    let mut clips_migrated = file_format_migrated;
    if !file_clips.is_empty() {
        config.clips = file_clips;
        if file_format_migrated {
            tracing::info!("registered-clips.dat の本文を暗号文形式へ移行する");
        }
    } else if !legacy_clips.is_empty() {
        config.clips = encrypt_legacy_toml_clips(legacy_clips);
        clips_migrated = true;
        tracing::info!("config.toml の [[clips]] を registered-clips.dat へ移行する");
    }

    if migrate_legacy_clip_images(&mut config.clips) {
        clips_migrated = true;
    }

    config.normalize_clips();

    (
        config,
        schema_migrated || clips_migrated,
        newer_version_clamped,
    )
}

/// 将来バージョン向け設定を検出した旨をログへ記録する
fn notify_newer_config_version_clamped() {
    tracing::warn!(
        "将来バージョン向けの設定を検出した。既存の項目は維持し、version を現行に揃えた"
    );
}

/// 破損した設定ファイルをバックアップする
///
/// # Returns
/// * `bool` - バックアップファイルの作成に成功した場合は `true`
fn backup_corrupted_config(config_path: &Path) -> bool {
    let backup_path = config_path.with_file_name("config.toml.bak");
    match fs::copy(config_path, &backup_path) {
        Ok(_) => {
            if let Err(e) = restrict_private_file_permissions(&backup_path) {
                tracing::warn!("設定バックアップのパーミッション設定に失敗: {:?}", e);
            }
            tracing::info!(
                "破損した設定をバックアップしました: {}",
                backup_path.display()
            );
            true
        }
        Err(e) => {
            tracing::warn!("設定ファイルのバックアップに失敗: {:?}", e);
            false
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::paths::get_config_file_path;

    /// 破損した `config.toml` はデフォルトへフォールバックし、`.bak` を作成すること
    #[test]
    fn load_parse_failure_falls_back_and_creates_backup() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("dir");
            }
            fs::write(&path, "mode = [[invalid").expect("write");

            let outcome = AppConfig::load();
            assert!(outcome.fallback.is_some());
            let fallback = outcome.fallback.expect("fallback");
            assert!(matches!(fallback.error, ConfigReloadError::Parse(_)));
            assert!(fallback.backup_created);
            assert!(path.with_file_name("config.toml.bak").exists());
            assert_eq!(outcome.config.mode, AppConfig::default().mode);
            assert!(outcome.unknown_keys.is_empty());
        });
    }

    /// 未知キーがあっても既知キーは読み込み、警告一覧に含めること
    #[test]
    fn load_keeps_known_values_and_reports_unknown_keys() {
        crate::test_helpers::with_temp_config_dir(|| {
            let path = get_config_file_path().expect("path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("dir");
            }
            fs::write(
                &path,
                r#"
version = 2
mode = "Trim"
interval_ms = 1000
typo_root = true

[hotkeys]
quick_selector = "Alt+Shift+S"
typo_hotkey = "Alt+Shift+Z"
"#,
            )
            .expect("write");

            let outcome = AppConfig::load();
            assert!(outcome.fallback.is_none());
            assert_eq!(outcome.config.mode, crate::refiner::RefineMode::Trim);
            assert_eq!(
                outcome.unknown_keys,
                vec!["hotkeys.typo_hotkey".to_string(), "typo_root".to_string(),]
            );

            let reloaded = AppConfig::reload_from_disk().expect("reload");
            assert_eq!(reloaded.config.mode, crate::refiner::RefineMode::Trim);
            assert_eq!(reloaded.unknown_keys, outcome.unknown_keys);
        });
    }

    /// `save` が既存設定を原子的に置き換え、一時ファイルを残さないこと
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

    /// フォールバック理由からユーザー向けメッセージを組み立てられるようにすること
    #[test]
    fn config_load_fallback_user_message_includes_backup_hint() {
        let with_backup = ConfigLoadFallback {
            error: ConfigReloadError::Parse("unexpected key".to_string()),
            backup_created: true,
        };
        let message = with_backup.user_message();
        assert!(message.contains("形式が不正"));
        assert!(message.contains("config.toml.bak"));

        let without_backup = ConfigLoadFallback {
            error: ConfigReloadError::Read("access denied".to_string()),
            backup_created: false,
        };
        assert!(
            without_backup
                .user_message()
                .contains("読み取れませんでした")
        );
        assert!(
            without_backup
                .user_message()
                .contains("デフォルト設定で起動")
        );
    }

    /// `config.toml` の `[[clips]]` は別ファイルへ移行され、TOML から除去されるようにすること
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
            let parsed = parse_config_toml(toml_str).expect("parse");
            assert_eq!(parsed.legacy_clips.len(), 1);
            assert!(parsed.config.clips.is_empty());
            assert!(parsed.unknown_keys.is_empty());

            let (config, migrated, newer_version_clamped) =
                finish_loading(parsed.config, parsed.legacy_clips);
            assert!(migrated);
            assert!(!newer_version_clamped);
            assert_eq!(config.clips.len(), 1);
            assert!(!config.clips[0].text_ciphertext.is_empty());
            let plain = super::super::registered_clips::decrypt_registered_clip_text(
                &config.clips[0].text_ciphertext,
            )
            .expect("decrypt");
            assert_eq!(plain.as_str(), "secret-body");

            config.save().expect("save");
            let content =
                fs::read_to_string(AppConfig::config_path().expect("path")).expect("read");
            assert!(!content.contains("[[clips]]"));
            assert!(!content.contains("secret-body"));

            let (file_clips, format_migrated) = load_registered_clips().expect("load clips file");
            assert!(!format_migrated);
            assert_eq!(file_clips.len(), 1);
            let plain = super::super::registered_clips::decrypt_registered_clip_text(
                &file_clips[0].text_ciphertext,
            )
            .expect("decrypt file");
            assert_eq!(plain.as_str(), "secret-body");
        });
    }
}

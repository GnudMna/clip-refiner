//! 設定ファイルの読み込み・再読み込み

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::super::paths::get_config_file_path;
use super::super::permissions::restrict_private_file_permissions;
use super::super::registered_clips::{
    load_registered_clips, migrate_legacy_clip_images, registered_clip_from_plain_text,
};
use super::super::types::{AppConfig, RegisteredClip};
use super::super::unknown_keys::{collect_unknown_config_keys, warn_unknown_config_keys};
use super::outcome::{ConfigDiskReload, ConfigLoadFallback, ConfigLoadOutcome, ConfigReloadError};
use crate::error::ConfigError;

impl AppConfig {
    /// 設定ファイルを読み込む
    ///
    /// 存在しない場合はデフォルト設定を生成し、説明コメント付きの `config.toml` を保存する
    /// 読み込みや解析に失敗した場合はデフォルト設定を返し、[`ConfigLoadOutcome::fallback`] に理由を格納する
    /// 解析に失敗した場合は元ファイルを `config.toml.bak` へ退避する
    /// 認識できないキーは [`ConfigLoadOutcome::unknown_keys`] に格納し、読み込み自体は継続する
    /// 移行手順が無い `version` はデフォルトへ置換せず、既存項目を維持する
    ///
    /// # Returns
    /// * `ConfigLoadOutcome` - 読み込んだ設定と、フォールバック時の理由・未知キー・未対応 version の有無
    pub fn load() -> ConfigLoadOutcome {
        let config_path = match get_config_file_path() {
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
                    unsupported_version_kept: false,
                };
            }
        };

        if !config_path.exists() {
            return ConfigLoadOutcome {
                config: Self::create_initial_config(&config_path),
                fallback: None,
                unknown_keys: Vec::new(),
                unsupported_version_kept: false,
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
                    unsupported_version_kept: false,
                };
            }
        };

        match parse_config_toml(&content) {
            Ok(parsed) => {
                warn_unknown_config_keys(&parsed.unknown_keys);
                let finished = finish_loading(parsed.config, parsed.legacy_clips);
                apply_version_notices(&finished);
                if finished.migrated
                    && !finished.unsupported_version_kept
                    && let Err(e) = finished.config.save()
                {
                    tracing::warn!("移行後の設定保存に失敗: {:?}", e);
                }
                ConfigLoadOutcome {
                    config: finished.config,
                    fallback: None,
                    unknown_keys: parsed.unknown_keys,
                    unsupported_version_kept: finished.unsupported_version_kept,
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
                    unsupported_version_kept: false,
                }
            }
        }
    }

    /// ディスク上の設定ファイルを再読み込みする
    ///
    /// スキーマ移行またはレガシー移行を行った場合は移行後の内容を保存する (未対応 version の場合は保存しない)
    ///
    /// # Returns
    /// * `Result<ConfigDiskReload, ConfigReloadError>` - 読み込んだ設定と移行・警告メタデータ
    ///
    /// # Errors
    /// 設定ファイルパスの取得失敗、ファイルが存在しない、読み取り失敗、TOML 解析失敗のいずれか
    pub fn reload_from_disk() -> Result<ConfigDiskReload, ConfigReloadError> {
        let config_path =
            get_config_file_path().map_err(|e| ConfigReloadError::Path(e.to_string()))?;

        if !config_path.exists() {
            return Err(ConfigReloadError::NotFound);
        }

        let content =
            fs::read_to_string(&config_path).map_err(|e| ConfigReloadError::Read(e.to_string()))?;

        let parsed =
            parse_config_toml(&content).map_err(|e| ConfigReloadError::Parse(e.to_string()))?;
        warn_unknown_config_keys(&parsed.unknown_keys);

        let finished = finish_loading(parsed.config, parsed.legacy_clips);
        apply_version_notices(&finished);
        if finished.migrated
            && !finished.unsupported_version_kept
            && let Err(e) = finished.config.save()
        {
            tracing::warn!("移行後の設定保存に失敗: {:?}", e);
        }

        Ok(ConfigDiskReload {
            config: finished.config,
            migrated: finished.migrated,
            newer_version_clamped: finished.newer_version_clamped,
            unsupported_version_kept: finished.unsupported_version_kept,
            unknown_keys: parsed.unknown_keys,
        })
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
// 解析・移行
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
fn parse_config_toml(content: &str) -> Result<ParsedConfigToml, ConfigError> {
    let mut value: toml::Value =
        toml::from_str(content).map_err(|e| ConfigError::serialize(e.to_string()))?;
    let unknown_keys = collect_unknown_config_keys(&value);
    drop_shadowed_legacy_keys(&mut value);
    let legacy_clips = value
        .get("clips")
        .and_then(|clips| clips.clone().try_into().ok())
        .unwrap_or_default();
    let config: AppConfig = value
        .try_into()
        .map_err(|e: toml::de::Error| ConfigError::serialize(e.to_string()))?;
    Ok(ParsedConfigToml {
        config,
        legacy_clips,
        unknown_keys,
    })
}

/// 旧キー名と現行キー名の対応 (テーブル名, 旧キー名, 現行キー名)
///
/// 現行キー名は読み込み時の別名として旧キー名も受け付けるため、両方があると重複扱いで解析に失敗する
const SHADOWED_LEGACY_KEYS: &[(&str, &str, &str)] = &[
    ("hotkeys", "selector", "quick_selector"),
    ("hotkeys", "pause", "auto_refine"),
    ("hotkeys", "paste_queue", "continuous_record_paste"),
    (
        "notification_settings",
        "notify_pause",
        "notify_auto_refine",
    ),
];

/// 現行キー名と同じテーブルにある旧キー名を取り除く
///
/// 旧版へ戻して保存した後に再び更新した場合や手編集で、旧キー名と現行キー名が同居することがある
/// 現行キー名の値を優先する
fn drop_shadowed_legacy_keys(value: &mut toml::Value) {
    for (table, legacy, current) in SHADOWED_LEGACY_KEYS {
        if let Some(table) = value.get_mut(*table).and_then(toml::Value::as_table_mut)
            && table.contains_key(*current)
            && table.remove(*legacy).is_some()
        {
            tracing::warn!("旧キー {legacy} を無視し、{current} の値を使う");
        }
    }
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
                template: false,
                counter: 0,
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

/// スキーマ移行と読み込み後フラグ
struct FinishedLoad {
    config: AppConfig,
    migrated: bool,
    newer_version_clamped: bool,
    unsupported_version_kept: bool,
}

/// スキーマ移行・登録クリップ読み込み・レガシー移行を行う
fn finish_loading(config: AppConfig, legacy_clips: Vec<LegacyTomlRegisteredClip>) -> FinishedLoad {
    let migration = config.prepare_loaded();
    let mut config = migration.config;
    let schema_migrated = migration.migrated;
    let newer_version_clamped = migration.newer_version_clamped;
    let unsupported_version_kept = migration.unsupported_version_kept;

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

    FinishedLoad {
        config,
        migrated: schema_migrated || clips_migrated,
        newer_version_clamped,
        unsupported_version_kept,
    }
}

/// 将来 version / 未対応 version のログを出す
fn apply_version_notices(finished: &FinishedLoad) {
    if finished.newer_version_clamped {
        tracing::warn!(
            "将来バージョン向けの設定を検出した。既存の項目は維持し、version を現行に揃えた"
        );
    }
    if finished.unsupported_version_kept {
        tracing::warn!(
            "{}",
            crate::config::format_unsupported_config_version_message()
        );
    }
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

    /// 旧キー名と現行キー名が同居していても解析に失敗せず、現行キー名の値を使うこと
    #[test]
    fn parse_prefers_current_key_over_legacy_alias() {
        let parsed = parse_config_toml(
            r#"
version = 5
mode = "Trim"
interval_ms = 1000

[notification_settings]
notify_pause = true
notify_auto_refine = false

[hotkeys]
pause = "Ctrl+Alt+P"
auto_refine = "Ctrl+Alt+R"
paste_queue = "Ctrl+Alt+Q"
continuous_record_paste = "Ctrl+Alt+X"
"#,
        )
        .unwrap_or_else(|e| panic!("旧キー同居の TOML を解析できること: {e:?}"));

        assert_eq!(parsed.config.hotkeys.auto_refine, "Ctrl+Alt+R");
        assert_eq!(parsed.config.hotkeys.continuous_record_paste, "Ctrl+Alt+X");
        assert!(!parsed.config.notification_settings.notify_auto_refine);
        // 旧キー名は既知キーのため未知キーとして報告しない
        assert!(parsed.unknown_keys.is_empty());
    }

    /// 旧キー名だけの場合は従来どおり現行キーとして読み込むこと
    #[test]
    fn parse_keeps_legacy_alias_without_current_key() {
        let parsed = parse_config_toml(
            r#"
version = 4
mode = "Trim"
interval_ms = 1000

[hotkeys]
pause = "Ctrl+Alt+P"
"#,
        )
        .unwrap_or_else(|e| panic!("旧キーのみの TOML を解析できること: {e:?}"));

        assert_eq!(parsed.config.hotkeys.auto_refine, "Ctrl+Alt+P");
    }

    /// 登録クリップファイルを読み込めなかった起動後の設定保存で、ファイルを削除しないこと
    #[test]
    fn save_after_clips_load_failure_keeps_registered_clips_file() {
        crate::test_helpers::with_temp_config_dir(|| {
            let dir = crate::config::paths::get_config_dir().expect("dir");
            let dat = dir.join("registered-clips.dat");
            AppConfig::default().save().expect("初回保存");
            fs::write(&dat, b"broken").expect("write");

            let outcome = AppConfig::load();
            assert!(outcome.config.clips.is_empty());
            assert!(outcome.fallback.is_none());
            assert_eq!(
                fs::read(dir.join("registered-clips.dat.bak")).expect("backup"),
                b"broken"
            );

            outcome.config.save().expect("save");
            assert_eq!(fs::read(&dat).expect("kept"), b"broken");
        });
    }

    /// 未知キーがあっても既知キーは読み込み、警告一覧に含めるようにすること
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

            let finished = finish_loading(parsed.config, parsed.legacy_clips);
            assert!(finished.migrated);
            assert!(!finished.newer_version_clamped);
            assert!(!finished.unsupported_version_kept);
            let config = finished.config;
            assert_eq!(config.clips.len(), 1);
            assert!(!config.clips[0].text_ciphertext.is_empty());
            let plain = super::super::super::registered_clips::decrypt_registered_clip_text(
                &config.clips[0].text_ciphertext,
            )
            .expect("decrypt");
            assert_eq!(plain.as_str(), "secret-body");

            config.save().expect("save");
            let content = fs::read_to_string(get_config_file_path().expect("path")).expect("read");
            assert!(!content.contains("[[clips]]"));
            assert!(!content.contains("secret-body"));

            let (file_clips, format_migrated) = load_registered_clips().expect("load clips file");
            assert!(!format_migrated);
            assert_eq!(file_clips.len(), 1);
            let plain = super::super::super::registered_clips::decrypt_registered_clip_text(
                &file_clips[0].text_ciphertext,
            )
            .expect("decrypt file");
            assert_eq!(plain.as_str(), "secret-body");
        });
    }
}

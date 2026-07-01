use super::favorites::{FavoriteMoveDirection, FavoriteToggleResult};
use super::hotkeys::HotkeySettings;
use super::monitor::MonitorMode;
use super::notification::NotificationSettings;
use super::regex::RegexSettings;
use super::registered_clip::{AddRegisteredClipError, RegisteredClip, ResolvedClip};

use crate::consts;
use crate::refiner::RefineMode;

use serde::{Deserialize, Serialize};

/// 設定ファイルに `version` が無い場合のデシリアライズ用デフォルト
fn default_config_version() -> u32 {
    consts::CONFIG_VERSION
}

/// 履歴の最大保持数を返す
///
/// # Returns
/// * `usize` - 履歴の最大保持数
fn default_history_limit() -> usize {
    consts::DEFAULT_HISTORY_LIMIT
}

// ======================================================================
// アプリケーション設定
// ======================================================================
/// アプリケーションの設定情報
///
/// TOML ファイルとして保存・読み込みされるアプリケーション全体の構成設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 設定ファイルのスキーマバージョン
    #[serde(default = "default_config_version")]
    pub version: u32,
    /// 最後に使用した(または常駐時に使用する)加工モード
    pub mode: RefineMode,
    /// 監視時に順に適用する加工モードの連鎖 (空の場合は `mode` のみを使用)
    #[serde(default)]
    pub pipeline: Vec<RefineMode>,
    /// 監視周期(ミリ秒)。ポーリング方式の場合に使用される。
    pub interval_ms: u64,
    /// 使用する監視方式(Polling または Event)
    #[serde(default)]
    pub monitor_mode: MonitorMode,
    /// 履歴機能が有効かどうか
    #[serde(default)]
    pub history_enabled: bool,
    /// クリップボード履歴の最大保持件数
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    /// 監視が一時停止されているかどうか
    #[serde(default)]
    pub is_paused: bool,
    /// 通知の内容設定
    #[serde(default)]
    pub notification_settings: NotificationSettings,
    /// グローバルホットキー設定
    #[serde(default)]
    pub hotkeys: HotkeySettings,
    /// 正規表現加工用のパターンと置換文字列
    #[serde(default)]
    pub regex: RegexSettings,
    /// クリップボードへコピーする登録クリップ (`registered-clips.dat` に保存)
    #[serde(skip)]
    pub clips: Vec<RegisteredClip>,
    /// お気に入り登録した変換モード (登録順)
    #[serde(default)]
    pub favorite_modes: Vec<RefineMode>,
}

impl Default for AppConfig {
    /// デフォルトのアプリケーション設定を生成する
    ///
    /// # Returns
    /// * `Self` - 標準的な動作環境のためのデフォルト設定
    fn default() -> Self {
        Self {
            version: consts::CONFIG_VERSION,
            mode: RefineMode::UrlDecode,
            pipeline: Vec::new(),
            interval_ms: 1000,
            monitor_mode: MonitorMode::default(),
            history_enabled: false,
            history_limit: consts::DEFAULT_HISTORY_LIMIT,
            is_paused: false,
            notification_settings: NotificationSettings::default(),
            hotkeys: HotkeySettings::default(),
            regex: RegexSettings::default(),
            clips: Vec::new(),
            favorite_modes: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 読み込み直後の後処理: スキーマ移行・値クランプ・ホットキー検証
    ///
    /// # Returns
    /// * `ConfigMigration` - 後処理済み設定と移行メタデータ
    pub fn prepare_loaded(self) -> super::super::migrate::ConfigMigration {
        let mut migration = super::super::migrate::migrate_config(self);
        migration.config.clamp_values();
        migration.config.normalize_clips();
        migration.config.normalize_favorite_modes();
        migration.config.normalize_pipeline();
        migration.config.hotkeys.fix_invalid();
        migration
    }

    /// 保存前の正規化: 数値クランプとスキーマバージョンを現行へ更新
    pub fn normalize(&mut self) {
        self.clamp_values();
        self.normalize_clips();
        self.normalize_favorite_modes();
        self.normalize_pipeline();
        self.version = consts::CONFIG_VERSION;
    }

    /// 数値項目を許容範囲内に収める
    pub(crate) fn clamp_values(&mut self) {
        self.history_limit = self
            .history_limit
            .clamp(consts::MIN_HISTORY_LIMIT, consts::MAX_HISTORY_LIMIT);
        self.interval_ms = self
            .interval_ms
            .clamp(consts::MIN_INTERVAL_MS, consts::MAX_INTERVAL_MS);
    }

    /// 登録クリップを許容範囲内に正規化する
    pub(crate) fn normalize_clips(&mut self) {
        use super::super::registered_images::{
            format_registered_image_label, load_registered_image, registered_image_exists,
        };
        use crate::security::{format_public_snippet, is_within_clipboard_limit};

        self.clips.retain(|entry| {
            if let Some(ref image_file) = entry.image_file {
                registered_image_exists(image_file)
            } else {
                !entry.text.trim().is_empty() && is_within_clipboard_limit(&entry.text)
            }
        });
        if self.clips.len() > consts::MAX_REGISTERED_CLIPS {
            self.clips.truncate(consts::MAX_REGISTERED_CLIPS);
        }
        for entry in &mut self.clips {
            if entry.label.trim().is_empty() {
                entry.label = if let Some(ref image_file) = entry.image_file {
                    load_registered_image(image_file).map_or_else(
                        |_| "登録画像".to_string(),
                        |(width, height, _)| format_registered_image_label(width, height),
                    )
                } else {
                    let preview = format_public_snippet(&entry.text, 20);
                    if preview.is_empty() {
                        "登録クリップ".to_string()
                    } else {
                        preview
                    }
                };
            }
            let char_count = entry.label.chars().count();
            if char_count > consts::MAX_REGISTERED_CLIP_LABEL_CHARS {
                entry.label = entry
                    .label
                    .chars()
                    .take(consts::MAX_REGISTERED_CLIP_LABEL_CHARS)
                    .collect();
            }
        }
    }

    /// 登録クリップをクイックセレクター向け JSON 配列へ変換する
    pub fn clips_to_json_list(&self) -> String {
        use crate::security::format_public_snippet;

        let list: Vec<serde_json::Value> = self
            .clips
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let preview = if entry.is_image() {
                    entry.label.clone()
                } else {
                    format_public_snippet(&entry.text, consts::REGISTERED_CLIP_PREVIEW_MAX_CHARS)
                };
                let thumbnail = entry.image_file.as_ref().and_then(|image_file| {
                    super::super::registered_images::registered_image_thumbnail_data_url(
                        image_file,
                        consts::SELECTOR_IMAGE_PREVIEW_MAX_DIMENSION,
                    )
                });
                let hover_preview = entry.image_file.as_ref().and_then(|image_file| {
                    super::super::registered_images::registered_image_hover_preview_data_url(
                        image_file,
                        consts::SELECTOR_IMAGE_HOVER_PREVIEW_MAX_DIMENSION,
                    )
                });
                entry.to_clip_selector_json(
                    index,
                    &preview,
                    thumbnail.as_deref(),
                    hover_preview.as_deref(),
                )
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
    }

    /// 指定インデックスの登録クリップ本文を返す
    pub fn registered_clip_text_at(&self, index: usize) -> Option<&str> {
        self.clips
            .get(index)
            .filter(|entry| !entry.is_image())
            .map(|entry| entry.text.as_str())
    }

    /// 指定インデックスの登録クリップボード内容を返す
    pub fn resolve_registered_clip(&self, index: usize) -> Option<ResolvedClip> {
        let entry = self.clips.get(index)?;
        if let Some(ref image_file) = entry.image_file {
            let (width, height, rgba) =
                super::super::registered_images::load_registered_image(image_file).ok()?;
            Some(ResolvedClip::Image {
                width,
                height,
                rgba,
            })
        } else {
            Some(ResolvedClip::Text(entry.text.clone()))
        }
    }

    /// クリップボードの内容を登録クリップとして追加する
    ///
    /// # Returns
    /// * `Ok(())` - 追加成功
    /// * `Err(AddRegisteredClipError)` - 空文字・サイズ超過・件数上限
    pub fn add_registered_clip(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), AddRegisteredClipError> {
        use crate::security::is_within_clipboard_limit;

        let text = text.into();
        if text.trim().is_empty() {
            return Err(AddRegisteredClipError::Empty);
        }
        if !is_within_clipboard_limit(&text) {
            return Err(AddRegisteredClipError::TooLarge);
        }
        if self.clips.len() >= consts::MAX_REGISTERED_CLIPS {
            return Err(AddRegisteredClipError::LimitReached);
        }

        self.clips.push(RegisteredClip {
            label: String::new(),
            text,
            image_file: None,
        });
        self.normalize_clips();
        Ok(())
    }

    /// クリップボードの画像を登録画像として追加する
    ///
    /// # Returns
    /// * `Ok(())` - 追加成功
    /// * `Err(AddRegisteredClipError)` - サイズ超過・件数上限・保存失敗
    pub fn add_registered_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), AddRegisteredClipError> {
        if self.clips.len() >= consts::MAX_REGISTERED_CLIPS {
            return Err(AddRegisteredClipError::LimitReached);
        }

        let image_file = super::super::registered_images::save_registered_image(
            width, height, rgba,
        )
        .map_err(|err| {
            crate::log_error!("登録画像の保存に失敗: {:?}", err);
            if err.to_string().contains("上限") {
                AddRegisteredClipError::ImageTooLarge
            } else {
                AddRegisteredClipError::ImageInvalid
            }
        })?;

        self.clips.push(RegisteredClip {
            label: String::new(),
            text: String::new(),
            image_file: Some(image_file),
        });
        self.normalize_clips();
        Ok(())
    }

    /// 指定インデックスの登録クリップを削除する
    ///
    /// # Returns
    /// * `bool` - 削除できた場合は `true`
    pub fn remove_registered_clip(&mut self, index: usize) -> bool {
        if index >= self.clips.len() {
            return false;
        }
        if let Some(image_file) = self.clips[index].image_file.clone() {
            super::super::registered_images::delete_registered_image(&image_file);
        }
        self.clips.remove(index);
        self.normalize_clips();
        true
    }

    /// 監視時に適用する加工モード列を返す
    ///
    /// `pipeline` が空の場合は `mode` のみ、それ以外は `pipeline` をそのまま返す
    pub fn effective_pipeline(&self) -> Vec<RefineMode> {
        if self.pipeline.is_empty() {
            vec![self.mode]
        } else {
            self.pipeline.clone()
        }
    }

    /// 加工パイプラインが有効かどうか (`pipeline` が空でない)
    pub fn is_pipeline_active(&self) -> bool {
        !self.pipeline.is_empty()
    }

    /// 加工パイプラインを許容範囲内に正規化する
    ///
    /// 画像出力モードは末尾1つのみ残し、それ以外の位置からは除去する
    pub(crate) fn normalize_pipeline(&mut self) {
        let image_mode = self
            .pipeline
            .iter()
            .rfind(|mode| mode.produces_image())
            .copied();
        self.pipeline.retain(|mode| !mode.produces_image());
        if let Some(image_mode) = image_mode {
            self.pipeline.push(image_mode);
        }

        if self.pipeline.len() > consts::MAX_PIPELINE_LENGTH {
            self.pipeline.truncate(consts::MAX_PIPELINE_LENGTH);
        }
    }

    /// お気に入り変換モードを許容範囲内に正規化する
    pub(crate) fn normalize_favorite_modes(&mut self) {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        self.favorite_modes.retain(|mode| {
            if seen.contains(mode) {
                false
            } else {
                seen.insert(*mode);
                true
            }
        });
        if self.favorite_modes.len() > consts::MAX_FAVORITE_MODES {
            self.favorite_modes.truncate(consts::MAX_FAVORITE_MODES);
        }
    }

    /// 指定モードがお気に入り登録済みかどうか
    pub fn is_favorite_mode(&self, mode: RefineMode) -> bool {
        self.favorite_modes.contains(&mode)
    }

    /// お気に入り変換モードの登録状態を切り替える
    ///
    /// # Returns
    /// * `FavoriteToggleResult` - 切り替え結果
    pub fn toggle_favorite_mode(&mut self, mode: RefineMode) -> FavoriteToggleResult {
        if let Some(index) = self.favorite_modes.iter().position(|m| *m == mode) {
            self.favorite_modes.remove(index);
            FavoriteToggleResult::Removed
        } else if self.favorite_modes.len() >= consts::MAX_FAVORITE_MODES {
            FavoriteToggleResult::LimitReached
        } else {
            self.favorite_modes.push(mode);
            FavoriteToggleResult::Added
        }
    }

    /// お気に入り変換モードの表示順を1つ移動する
    ///
    /// # Returns
    /// * `bool` - 移動できた場合は `true`
    pub fn move_favorite_mode(
        &mut self,
        mode: RefineMode,
        direction: FavoriteMoveDirection,
    ) -> bool {
        let Some(index) = self.favorite_modes.iter().position(|m| *m == mode) else {
            return false;
        };
        let target = match direction {
            FavoriteMoveDirection::Up if index > 0 => index - 1,
            FavoriteMoveDirection::Down if index + 1 < self.favorite_modes.len() => index + 1,
            _ => return false,
        };
        self.favorite_modes.swap(index, target);
        true
    }

    /// クイックセレクター向けの変換モード JSON 配列を生成する
    pub fn modes_to_json_list(&self) -> String {
        RefineMode::to_json_list(&self.favorite_modes)
    }
}

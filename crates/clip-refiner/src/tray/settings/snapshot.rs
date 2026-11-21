use serde::Serialize;

use super::model::SettingsWritable;
use crate::config::AppConfig;
use crate::consts;
use crate::refiner::{REGEX_PREVIEW_SAMPLE, RefineMode};
use crate::tray::hotkey::settings::shortcut_list_entries;
use crate::tray::selector::{clip_selector_json, clip_selector_value, quick_selector_value};

// ======================================================================
// UI 制約
// ======================================================================
/// 設定ウィンドウ向けの件数・サイズ上限
///
/// JavaScript 側のクランプと表示文言はこの値のみを参照する
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLimits {
    /// 履歴の最小保持件数
    pub history_min: usize,
    /// 履歴の最大保持件数
    pub history_max: usize,
    /// 連続記録の最小保持件数
    pub continuous_record_min: usize,
    /// 連続記録の最大保持件数
    pub continuous_record_max: usize,
    /// ポーリング間隔の最小値 (ミリ秒)
    pub interval_min_ms: u64,
    /// ポーリング間隔の最大値 (ミリ秒)
    pub interval_max_ms: u64,
    /// セレクター `WebView` アイドル破棄分数の下限
    pub selector_webview_idle_min_minutes: u64,
    /// セレクター `WebView` アイドル破棄分数の上限
    pub selector_webview_idle_max_minutes: u64,
    /// 加工パイプラインの最大段数
    pub max_pipeline_length: usize,
    /// お気に入り変換モードの最大件数
    pub max_favorite_modes: usize,
    /// 登録クリップの最大件数
    pub max_registered_clips: usize,
    /// 正規表現パターンの最大バイト数
    pub max_regex_pattern_bytes: usize,
    /// 正規表現プリセットの最大件数
    pub max_regex_presets: usize,
    /// 正規表現プリセットあたりの最大ステップ数
    pub max_regex_steps_per_preset: usize,
}

impl SettingsLimits {
    /// 定数から UI 制約を生成する
    ///
    /// # Returns
    /// * `Self` - 設定ウィンドウ向けの件数・サイズ上限
    pub fn from_consts() -> Self {
        Self {
            history_min: consts::MIN_HISTORY_LIMIT,
            history_max: consts::MAX_HISTORY_LIMIT,
            continuous_record_min: consts::MIN_CONTINUOUS_RECORD_LIMIT,
            continuous_record_max: consts::MAX_CONTINUOUS_RECORD_LIMIT,
            interval_min_ms: consts::MIN_INTERVAL_MS,
            interval_max_ms: consts::MAX_INTERVAL_MS,
            selector_webview_idle_min_minutes: consts::MIN_SELECTOR_WEBVIEW_IDLE_MINUTES,
            selector_webview_idle_max_minutes: consts::MAX_SELECTOR_WEBVIEW_IDLE_MINUTES,
            max_pipeline_length: consts::MAX_PIPELINE_LENGTH,
            max_favorite_modes: consts::MAX_FAVORITE_MODES,
            max_registered_clips: consts::MAX_REGISTERED_CLIPS,
            max_regex_pattern_bytes: consts::MAX_REGEX_PATTERN_BYTES,
            max_regex_presets: consts::MAX_REGEX_PRESETS,
            max_regex_steps_per_preset: consts::MAX_REGEX_STEPS_PER_PRESET,
        }
    }
}

// ======================================================================
// 設定スナップショット
// ======================================================================
/// 設定ウィンドウの初期表示用データ
///
/// 読み書き項目は `SettingsWritable` に集約し、表示専用項目のみここで保持する
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    /// 設定ウィンドウで読み書きする項目
    #[serde(flatten)]
    pub writable: SettingsWritable,
    /// アプリバージョン (Cargo パッケージ版)
    pub app_version: &'static str,
    /// ポータブルモードで動作しているか
    pub portable_mode: bool,
    /// 正規表現プレビュー用サンプル文字列 (`REGEX_PREVIEW_SAMPLE` と同一)
    pub regex_preview_sample: &'static str,
    /// お気に入り変換モード (登録順)
    pub favorite_modes: Vec<RefineMode>,
    /// 変換モード一覧 (クイックセレクタと同形式)
    pub modes: serde_json::Value,
    /// 登録クリップ一覧
    pub clips: serde_json::Value,
    /// グローバルホットキー一覧 (表示用プレビュー)
    pub shortcuts: Vec<crate::tray::hotkey::settings::ShortcutEntry>,
    /// UI 向けの件数・サイズ上限
    pub limits: SettingsLimits,
}

impl SettingsSnapshot {
    /// 現在の設定からスナップショットを生成する
    ///
    /// # Arguments
    /// * `config` - 現在のアプリケーション設定
    ///
    /// # Returns
    /// * `Self` - 設定ウィンドウの初期表示用データ
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            writable: SettingsWritable::from_config(config),
            app_version: env!("CARGO_PKG_VERSION"),
            portable_mode: crate::config::is_portable_mode(),
            regex_preview_sample: REGEX_PREVIEW_SAMPLE,
            favorite_modes: config.favorite_modes.clone(),
            modes: quick_selector_value(config),
            clips: clip_selector_value(config),
            shortcuts: shortcut_list_entries(&config.hotkeys, &config.favorite_modes),
            limits: SettingsLimits::from_consts(),
        }
    }

    /// JSON 文字列へシリアライズする
    ///
    /// # Arguments
    /// * `self` - この `SettingsSnapshot`
    ///
    /// # Returns
    /// * `String` - スナップショットの JSON 文字列 (失敗時は `"{}"`)
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 登録クリップ一覧の JSON 文字列を生成する
    ///
    /// # Arguments
    /// * `config` - 現在のアプリケーション設定
    ///
    /// # Returns
    /// * `String` - 登録クリップ一覧 JSON
    pub fn clips_json(config: &AppConfig) -> String {
        clip_selector_json(config)
    }

    /// お気に入りとショートカット一覧の JSON 文字列を生成する
    ///
    /// # Arguments
    /// * `config` - 現在のアプリケーション設定
    ///
    /// # Returns
    /// * `String` - お気に入りモードとショートカット一覧 JSON
    pub fn favorites_and_shortcuts_json(config: &AppConfig) -> String {
        serde_json::json!({
            "favoriteModes": config.favorite_modes,
            "hotkeys": config.hotkeys,
            "shortcuts": shortcut_list_entries(&config.hotkeys, &config.favorite_modes),
        })
        .to_string()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, NotificationSettings, RegexSettings};
    use crate::consts;
    use crate::refiner::RefineMode;

    /// トップレベルとネスト設定は camelCase で JS と整合すること
    #[test]
    fn snapshot_json_matches_js_field_names() {
        let mut notification_settings = NotificationSettings::default();
        notification_settings.enabled = true;
        notification_settings.notify_process = false;
        notification_settings.notify_result = true;
        notification_settings.notify_auto_refine = false;
        notification_settings.notify_history_restore = true;
        notification_settings.notify_ocr = false;
        notification_settings.notify_registered_clip = true;
        notification_settings.notify_favorites = false;
        notification_settings.notify_continuous_record = true;
        let config = AppConfig {
            notification_settings,
            regex: RegexSettings::from_step(crate::config::RegexStep {
                pattern: "a".to_string(),
                replacement: "b".to_string(),
                case_insensitive: true,
                multiline: false,
            }),
            ..AppConfig::default()
        };

        let snapshot = SettingsSnapshot::from_config(&config);
        let json: serde_json::Value =
            serde_json::from_str(&snapshot.to_json()).expect("スナップショット JSON のパース");

        assert_eq!(json["monitorMode"], "Event");
        assert_eq!(json["notificationSettings"]["enabled"], true);
        assert_eq!(json["notificationSettings"]["notifyProcess"], false);
        assert_eq!(json["notificationSettings"]["notifyResult"], true);
        assert_eq!(json["notificationSettings"]["notifyHistoryRestore"], true);
        assert_eq!(json["notificationSettings"]["notifyOcr"], false);
        assert_eq!(json["notificationSettings"]["notifyRegisteredClip"], true);
        assert_eq!(json["regex"]["activePresetId"], "default");
        assert_eq!(
            json["regex"]["presets"][0]["steps"][0]["caseInsensitive"],
            true
        );
        assert_eq!(json["limits"]["historyMax"], consts::MAX_HISTORY_LIMIT);
        assert_eq!(json["limits"]["intervalMinMs"], consts::MIN_INTERVAL_MS);
        assert_eq!(json["limits"]["intervalMaxMs"], consts::MAX_INTERVAL_MS);
        assert_eq!(json["regexPreviewSample"], REGEX_PREVIEW_SAMPLE);
        assert_eq!(json["appVersion"], env!("CARGO_PKG_VERSION"));
        assert!(json["portableMode"].is_boolean());
        assert_eq!(json["hotkeys"]["quickSelector"], "Alt+Shift+S");
        assert!(json["hotkeys"]["favoriteModeSlots"].is_array());
        assert!(json["notificationSettings"].get("notify_mode").is_none());
        assert!(json["regex"].get("case_insensitive").is_none());
        assert!(json["hotkeys"].get("quick_selector").is_none());
    }

    /// `clips_json` が登録クリップ一覧を JSON 配列として返すようにすること
    #[test]
    fn clips_json_serializes_registered_clips() {
        let mut config = AppConfig::default();
        config
            .add_registered_clip("hello")
            .expect("登録クリップの追加");

        let json: serde_json::Value = serde_json::from_str(&SettingsSnapshot::clips_json(&config))
            .expect("clips JSON のパース");

        assert_eq!(json.as_array().map(Vec::len), Some(1));
        assert_eq!(json[0]["preview"], "hello");
    }

    /// `favorites_and_shortcuts_json` がお気に入りとショートカットを返すようにすること
    #[test]
    fn favorites_and_shortcuts_json_contains_expected_fields() {
        let config = AppConfig {
            favorite_modes: vec![RefineMode::Trim],
            ..AppConfig::default()
        };

        let json: serde_json::Value =
            serde_json::from_str(&SettingsSnapshot::favorites_and_shortcuts_json(&config))
                .expect("favorites JSON のパース");

        assert_eq!(json["favoriteModes"], serde_json::json!(["Trim"]));
        assert!(json["shortcuts"].is_array());
        assert!(json["hotkeys"].is_object());
        assert_eq!(json["hotkeys"]["quickSelector"], "Alt+Shift+S");
    }
}

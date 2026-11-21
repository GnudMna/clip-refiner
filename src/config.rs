use std::fs;
use std::path::{Path, PathBuf};

use crate::consts;
use crate::hotkey_binding::parse_hotkey_binding;
use crate::refiner::RefineMode;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ======================================================================
// 監視モード
// ======================================================================
/// クリップボードの監視方式
///
/// クリップボードの更新を検知するための異なるアプローチを提供する
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// 一定間隔でクリップボードの内容を確認するポーリング方式
    /// すべてのプラットフォームで動作する基本的な監視モード
    #[default]
    Polling,
    /// OSの変更トークンを監視する方式
    /// クリップボード本文の定期読み取りを避け、低遅延かつ低CPU負荷で動作する
    Event,
}

// ======================================================================
// 通知設定
// ======================================================================
/// 通知の内容に関する設定
///
/// どのタイミングでどのような通知を表示するかを制御する
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 成功通知機能全体の有効/無効スイッチ
    #[serde(default)]
    pub enabled: bool,
    /// 実行されたモード名を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_mode: bool,
    /// 加工結果を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_result: bool,
    /// 一時停止の切り替えを通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_pause: bool,
}

impl Default for NotificationSettings {
    /// デフォルトの通知設定を生成する
    ///
    /// # Returns
    /// * `Self` - 通知オフ・各サブ設定はオンのデフォルト設定
    fn default() -> Self {
        Self {
            enabled: false,
            notify_mode: true,
            notify_result: true,
            notify_pause: true,
        }
    }
}

// ======================================================================
// ホットキー設定
// ======================================================================
/// グローバルホットキーの割り当て
///
/// 各フィールドは `Alt+Shift+S` 形式の文字列で指定する
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeySettings {
    /// クイックセレクターの表示・非表示
    #[serde(default = "default_hotkey_selector")]
    pub selector: String,
    /// 成功通知のON/OFF切替
    #[serde(default = "default_hotkey_notification")]
    pub notification: String,
    /// 監視の一時停止・再開
    #[serde(default = "default_hotkey_pause")]
    pub pause: String,
    /// アプリケーションの終了
    #[serde(default = "default_hotkey_quit")]
    pub quit: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            selector: default_hotkey_selector(),
            notification: default_hotkey_notification(),
            pause: default_hotkey_pause(),
            quit: default_hotkey_quit(),
        }
    }
}

impl HotkeySettings {
    /// ショートカット一覧表示用の文字列を生成する
    pub fn shortcut_list_text(&self) -> String {
        format!(
            "{}: クイックセレクター\n{}: 成功通知の切替\n{}: 一時停止/再開\n{}: 終了",
            self.selector, self.notification, self.pause, self.quit
        )
    }

    /// 不正なホットキー文字列をデフォルト値へ置き換える
    pub fn fix_invalid(&mut self) {
        fix_hotkey_field(
            &mut self.selector,
            consts::DEFAULT_HOTKEY_SELECTOR,
            "selector",
        );
        fix_hotkey_field(
            &mut self.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
        );
        fix_hotkey_field(&mut self.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause");
        fix_hotkey_field(&mut self.quit, consts::DEFAULT_HOTKEY_QUIT, "quit");
    }
}

/// 不正なホットキー文字列をデフォルト値へ置き換える
///
/// # Arguments
/// * `field` - 不正なホットキー文字列
/// * `default` - デフォルトホットキー文字列
/// * `label` - ホットキー設定のラベル
fn fix_hotkey_field(field: &mut String, default: &str, label: &str) {
    if parse_hotkey_binding(field).is_err() {
        crate::log_warn!(
            "ホットキー設定 '{label}' が無効なためデフォルト '{default}' に置き換える (指定値: '{field}')"
        );
        *field = default.to_string();
    }
}

/// クイックセレクターのデフォルトホットキーを返す
///
/// # Returns
/// * `String` - クイックセレクターのデフォルトホットキー
fn default_hotkey_selector() -> String {
    consts::DEFAULT_HOTKEY_SELECTOR.to_string()
}

/// 成功通知のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 成功通知のデフォルトホットキー
fn default_hotkey_notification() -> String {
    consts::DEFAULT_HOTKEY_NOTIFICATION.to_string()
}

/// 一時停止のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 一時停止のデフォルトホットキー
fn default_hotkey_pause() -> String {
    consts::DEFAULT_HOTKEY_PAUSE.to_string()
}

/// 終了のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 終了のデフォルトホットキー
fn default_hotkey_quit() -> String {
    consts::DEFAULT_HOTKEY_QUIT.to_string()
}

/// 設定ファイルのバージョンを返す
///
/// # Returns
/// * `u32` - 設定ファイルのバージョン
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
/// JSONファイルとして保存・読み込みされるアプリケーション全体の構成設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 設定ファイルのスキーマバージョン
    #[serde(default = "default_config_version")]
    pub version: u32,
    /// 最後に使用した(または常駐時に使用する)加工モード
    pub mode: RefineMode,
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
            interval_ms: 1000,
            monitor_mode: MonitorMode::default(),
            history_enabled: false,
            history_limit: consts::DEFAULT_HISTORY_LIMIT,
            is_paused: false,
            notification_settings: NotificationSettings::default(),
            hotkeys: HotkeySettings::default(),
        }
    }
}

// ======================================================================
// 設定ファイル操作
// ======================================================================
impl AppConfig {
    /// 数値項目を許容範囲内に収め、スキーマバージョンを更新する
    pub fn normalize(&mut self) {
        self.version = consts::CONFIG_VERSION;
        self.history_limit = self
            .history_limit
            .clamp(consts::MIN_HISTORY_LIMIT, consts::MAX_HISTORY_LIMIT);
        self.interval_ms = self
            .interval_ms
            .clamp(consts::MIN_INTERVAL_MS, consts::MAX_INTERVAL_MS);
    }

    /// 設定ファイルの保存先パスをシステムOSに合わせて取得する
    ///
    /// # Returns
    /// * `Result<PathBuf>` - 設定ファイルの完全なパス
    fn config_path() -> Result<PathBuf> {
        let config_dir = get_config_dir()?;
        fs::create_dir_all(&config_dir).context("設定ディレクトリの作成に失敗しました")?;
        Ok(config_dir.join("config.json"))
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

// ======================================================================
// 設定ディレクトリ
// ======================================================================
/// 設定ディレクトリのパスを取得する
///
/// OSに応じたアプリケーション設定ディレクトリのパスを計算する
/// Windows は `ClipRefiner`、Linux/macOS は `clip-refiner` を使用する
///
/// # Returns
/// * `Result<PathBuf>` - OSに応じた設定ディレクトリのパス
pub fn get_config_dir() -> Result<PathBuf> {
    let base_dirs =
        directories::BaseDirs::new().context("システムディレクトリの取得に失敗しました")?;

    #[cfg(windows)]
    let dir_name = consts::APP_NAME;

    #[cfg(not(windows))]
    let dir_name = consts::APP_NAME_KEBAB;

    Ok(base_dirs.config_dir().join(dir_name))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// AppConfig のデフォルト値が正しいこと
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.version, consts::CONFIG_VERSION);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.mode, RefineMode::UrlDecode);
        assert_eq!(config.history_limit, consts::DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys, HotkeySettings::default());
    }

    /// AppConfig のシリアライズ/デシリアライズ往復
    #[test]
    fn test_app_config_serde() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            serde_json::from_str(&json).expect("AppConfig のデシリアライズに失敗");
        assert_eq!(config.interval_ms, decoded.interval_ms);
        assert_eq!(config.mode, decoded.mode);
        assert_eq!(config.history_limit, decoded.history_limit);
        assert_eq!(config.hotkeys, decoded.hotkeys);
    }

    /// NotificationSettings のデフォルト値が正しいこと
    #[test]
    fn test_notification_settings_default() {
        let ns = NotificationSettings::default();
        assert!(!ns.enabled, "enabled のデフォルトは false");
        assert!(ns.notify_mode);
        assert!(ns.notify_result);
        assert!(ns.notify_pause);
    }

    /// 古い設定 JSON (show_success_notification フィールドあり) を読んでも
    /// デフォルト値でデシリアライズできること
    #[test]
    fn test_app_config_backward_compat_old_field() {
        let old_json = r#"{
            "mode": "UrlDecode",
            "interval_ms": 1000,
            "show_success_notification": true
        }"#;
        let config: AppConfig =
            serde_json::from_str(old_json).expect("後方互換 JSON のデシリアライズに失敗");
        assert_eq!(config.interval_ms, 1000);
        assert!(!config.notification_settings.enabled);
        assert_eq!(config.history_limit, consts::DEFAULT_HISTORY_LIMIT);
        assert_eq!(config.hotkeys.selector, consts::DEFAULT_HOTKEY_SELECTOR);
    }

    /// notification_settings.enabled が JSON に保存・復元されること
    #[test]
    fn test_notification_settings_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.notification_settings.enabled = true;
        config.notification_settings.notify_result = false;

        let json = serde_json::to_string(&config).expect("AppConfig のシリアライズに失敗");
        let decoded: AppConfig =
            serde_json::from_str(&json).expect("AppConfig のデシリアライズに失敗");
        assert!(decoded.notification_settings.enabled);
        assert!(!decoded.notification_settings.notify_result);
    }

    /// normalize が範囲外の値をクランプすること
    #[test]
    fn test_app_config_normalize_clamps() {
        let mut config = AppConfig::default();
        config.history_limit = 999;
        config.interval_ms = 10;

        config.normalize();

        assert_eq!(config.history_limit, consts::MAX_HISTORY_LIMIT);
        assert_eq!(config.interval_ms, consts::MIN_INTERVAL_MS);
    }

    /// fix_invalid が不正なホットキーをデフォルトへ置き換えること
    #[test]
    fn test_hotkey_settings_fix_invalid() {
        let mut hotkeys = HotkeySettings {
            selector: "Bad+Key".to_string(),
            ..HotkeySettings::default()
        };
        hotkeys.fix_invalid();
        assert_eq!(hotkeys.selector, consts::DEFAULT_HOTKEY_SELECTOR);
    }
}

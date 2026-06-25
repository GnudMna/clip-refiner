use crate::consts;
use crate::hotkey_binding::parse_hotkey_binding;
use crate::refiner::RefineMode;

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
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 成功通知機能全体の有効/無効スイッチ
    #[serde(default)]
    pub enabled: bool,
    /// 実行されたモード名を通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_mode: bool,
    /// 通知にクリップボードの内容 (加工結果) を含めるかどうか
    #[serde(default)]
    pub notify_result: bool,
    /// 一時停止の切り替えを通知するかどうか
    #[serde(default = "consts::default_true")]
    pub notify_pause: bool,
}

impl Default for NotificationSettings {
    /// デフォルトの通知設定を生成する
    ///
    /// # Returns
    /// * `Self` - 通知オフ・内容表示オフ・その他サブ設定はオンのデフォルト設定
    fn default() -> Self {
        Self {
            enabled: false,
            notify_mode: true,
            notify_result: false,
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
    #[serde(default = "default_hotkey_quick_selector", alias = "selector")]
    pub quick_selector: String,
    /// 成功通知のON/OFF切替
    #[serde(default = "default_hotkey_notification")]
    pub notification: String,
    /// 監視の一時停止・再開
    #[serde(default = "default_hotkey_pause")]
    pub pause: String,
    /// アプリケーションの終了
    #[serde(default = "default_hotkey_quit")]
    pub quit: String,
    /// 直近の加工を取り消す
    #[serde(default = "default_hotkey_undo")]
    pub undo: String,
    /// 登録文字列セレクターの表示・非表示
    #[serde(default = "default_hotkey_text_selector")]
    pub text_selector: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            quick_selector: default_hotkey_quick_selector(),
            notification: default_hotkey_notification(),
            pause: default_hotkey_pause(),
            quit: default_hotkey_quit(),
            undo: default_hotkey_undo(),
            text_selector: default_hotkey_text_selector(),
        }
    }
}

impl HotkeySettings {
    /// ショートカット一覧表示用の文字列を生成する
    pub fn shortcut_list_text(&self) -> String {
        format!(
            "{}: クイックセレクター\n{}: 登録文字列セレクター\n{}: 成功通知の切替\n{}: 一時停止/再開\n{}: 加工の取り消し\n{}: 終了",
            self.quick_selector,
            self.text_selector,
            self.notification,
            self.pause,
            self.undo,
            self.quit
        )
    }

    /// 不正なホットキー文字列をデフォルト値へ置き換える
    pub fn fix_invalid(&mut self) {
        fix_hotkey_field(
            &mut self.quick_selector,
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            "quick_selector",
        );
        fix_hotkey_field(
            &mut self.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
        );
        fix_hotkey_field(&mut self.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause");
        fix_hotkey_field(&mut self.quit, consts::DEFAULT_HOTKEY_QUIT, "quit");
        fix_hotkey_field(&mut self.undo, consts::DEFAULT_HOTKEY_UNDO, "undo");
        fix_hotkey_field(
            &mut self.text_selector,
            consts::DEFAULT_HOTKEY_TEXT_SELECTOR,
            "text_selector",
        );
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
fn default_hotkey_quick_selector() -> String {
    consts::DEFAULT_HOTKEY_QUICK_SELECTOR.to_string()
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

/// 加工取り消しのデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 加工取り消しのデフォルトホットキー
fn default_hotkey_undo() -> String {
    consts::DEFAULT_HOTKEY_UNDO.to_string()
}

/// 登録文字列セレクターのデフォルトホットキーを返す
fn default_hotkey_text_selector() -> String {
    consts::DEFAULT_HOTKEY_TEXT_SELECTOR.to_string()
}

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
// 登録文字列
// ======================================================================
/// クリップボードへコピーするための登録文字列
///
/// `config.toml` の `[[texts]]` セクションとして保存される
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredText {
    /// 一覧表示用のラベル
    pub label: String,
    /// クリップボードへコピーする本文
    pub text: String,
}

impl RegisteredText {
    /// テキストセレクター向けの JSON オブジェクトを生成する
    pub fn to_text_selector_json(&self, index: usize, preview: &str) -> serde_json::Value {
        serde_json::json!({
            "id": index.to_string(),
            "label": self.label,
            "preview": preview,
        })
    }
}

// ======================================================================
// 正規表現設定
// ======================================================================
/// 正規表現加工モード用のパターンと置換文字列
///
/// `config.toml` の `[regex]` セクションとして保存される
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegexSettings {
    /// 正規表現パターン
    #[serde(default)]
    pub pattern: String,
    /// 置換文字列 (`RegexReplace` で使用。キャプチャグループは `$1` 形式)
    #[serde(default)]
    pub replacement: String,
    /// 大文字小文字を無視する (`(?i)` 相当)
    #[serde(default)]
    pub case_insensitive: bool,
    /// 複数行モード (`(?m)` 相当)
    #[serde(default)]
    pub multiline: bool,
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
    /// クリップボードへコピーする登録文字列
    #[serde(default)]
    pub texts: Vec<RegisteredText>,
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
            regex: RegexSettings::default(),
            texts: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 読み込み直後の後処理: スキーマ移行・値クランプ・ホットキー検証
    ///
    /// # Returns
    /// * `(Self, bool)` - 後処理済み設定と、スキーマ移行が実行されたかどうか
    pub fn prepare_loaded(self) -> (Self, bool) {
        let migration = super::migrate::migrate_config(self);
        let mut config = migration.config;
        config.clamp_values();
        config.normalize_texts();
        config.hotkeys.fix_invalid();
        (config, migration.migrated)
    }

    /// 保存前の正規化: 数値クランプとスキーマバージョンを現行へ更新
    pub fn normalize(&mut self) {
        self.clamp_values();
        self.normalize_texts();
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

    /// 登録文字列を許容範囲内に正規化する
    pub(crate) fn normalize_texts(&mut self) {
        use crate::security::{format_public_snippet, is_within_clipboard_limit};

        self.texts.retain(|entry| {
            !entry.text.trim().is_empty() && is_within_clipboard_limit(&entry.text)
        });
        if self.texts.len() > consts::MAX_REGISTERED_TEXTS {
            self.texts.truncate(consts::MAX_REGISTERED_TEXTS);
        }
        for entry in &mut self.texts {
            if entry.label.trim().is_empty() {
                let preview = format_public_snippet(&entry.text, 20);
                entry.label = if preview.is_empty() {
                    "登録文字列".to_string()
                } else {
                    preview
                };
            }
            let char_count = entry.label.chars().count();
            if char_count > consts::MAX_REGISTERED_TEXT_LABEL_CHARS {
                entry.label = entry
                    .label
                    .chars()
                    .take(consts::MAX_REGISTERED_TEXT_LABEL_CHARS)
                    .collect();
            }
        }
    }

    /// 登録文字列をクイックセレクター向け JSON 配列へ変換する
    pub fn texts_to_json_list(&self) -> String {
        use crate::security::format_public_snippet;

        let list: Vec<serde_json::Value> = self
            .texts
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let preview =
                    format_public_snippet(&entry.text, consts::REGISTERED_TEXT_PREVIEW_MAX_CHARS);
                entry.to_text_selector_json(index, &preview)
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
    }

    /// 指定インデックスの登録文字列本文を返す
    pub fn registered_text_at(&self, index: usize) -> Option<&str> {
        self.texts.get(index).map(|entry| entry.text.as_str())
    }

    /// クリップボードの内容を登録文字列として追加する
    ///
    /// # Returns
    /// * `Ok(())` - 追加成功
    /// * `Err(AddRegisteredTextError)` - 空文字・サイズ超過・件数上限
    pub fn add_registered_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), AddRegisteredTextError> {
        use crate::security::is_within_clipboard_limit;

        let text = text.into();
        if text.trim().is_empty() {
            return Err(AddRegisteredTextError::Empty);
        }
        if !is_within_clipboard_limit(&text) {
            return Err(AddRegisteredTextError::TooLarge);
        }
        if self.texts.len() >= consts::MAX_REGISTERED_TEXTS {
            return Err(AddRegisteredTextError::LimitReached);
        }

        self.texts.push(RegisteredText {
            label: String::new(),
            text,
        });
        self.normalize_texts();
        Ok(())
    }

    /// 指定インデックスの登録文字列を削除する
    ///
    /// # Returns
    /// * `bool` - 削除できた場合は `true`
    pub fn remove_registered_text(&mut self, index: usize) -> bool {
        if index >= self.texts.len() {
            return false;
        }
        self.texts.remove(index);
        self.normalize_texts();
        true
    }
}

// ======================================================================
// 登録文字列の追加
// ======================================================================
/// 登録文字列の追加に失敗した理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddRegisteredTextError {
    /// 空文字または空白のみ
    Empty,
    /// クリップボード上限を超える
    TooLarge,
    /// 登録件数の上限に達している
    LimitReached,
}

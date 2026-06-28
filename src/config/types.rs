use crate::consts;
use crate::hotkey_binding::parse_hotkey_binding;
use crate::refiner::RefineMode;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

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
    /// 画面範囲選択 OCR の開始
    #[serde(default = "default_hotkey_ocr")]
    pub ocr: String,
    /// お気に入り変換モード用ホットキー (登録順インデックスに対応。空文字で無効)
    #[serde(default)]
    pub favorite_mode_slots: Vec<String>,
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
            ocr: default_hotkey_ocr(),
            favorite_mode_slots: Vec::new(),
        }
    }
}

impl HotkeySettings {
    /// ショートカット一覧表示用の文字列を生成する
    pub fn shortcut_list_text(&self, favorite_modes: &[RefineMode]) -> String {
        let mut lines = vec![
            format!("{}: クイックセレクター", self.quick_selector),
            format!("{}: 登録文字列セレクター", self.text_selector),
            format!("{}: 画面 OCR", self.ocr),
            format!("{}: 成功通知の切替", self.notification),
            format!("{}: 一時停止/再開", self.pause),
            format!("{}: 加工の取り消し", self.undo),
            format!("{}: 終了", self.quit),
        ];
        for (index, mode) in favorite_modes.iter().enumerate() {
            if let Some(binding) = self.favorite_slot_binding(index) {
                lines.push(format!("{binding}: {} (お気に入り)", mode.label()));
            }
        }
        lines.join("\n")
    }

    /// お気に入りスロットのホットキー文字列を返す
    ///
    /// 設定で空文字が指定されたスロットは `None` を返す
    pub fn favorite_slot_binding(&self, index: usize) -> Option<String> {
        if index >= consts::MAX_FAVORITE_MODES {
            return None;
        }
        if let Some(binding) = self.favorite_mode_slots.get(index) {
            if binding.is_empty() {
                return None;
            }
            return Some(binding.clone());
        }
        default_favorite_slot_binding(index)
    }

    /// お気に入りスロット用ホットキーを解決する
    ///
    /// 固定ホットキーおよび先に割り当てたスロットと重複する割り当ては除外する
    pub fn resolve_favorite_slot_hotkeys(
        &self,
        favorite_count: usize,
        reserved: &[global_hotkey::hotkey::HotKey],
    ) -> Vec<(usize, global_hotkey::hotkey::HotKey)> {
        use std::collections::HashSet;

        let mut used_ids: HashSet<u32> = reserved
            .iter()
            .map(global_hotkey::hotkey::HotKey::id)
            .collect();
        let mut resolved = Vec::new();

        for index in 0..favorite_count.min(consts::MAX_FAVORITE_MODES) {
            let Some(binding) = self.favorite_slot_binding(index) else {
                continue;
            };
            let hotkey = match parse_hotkey_binding(&binding) {
                Ok(hotkey) => hotkey,
                Err(e) => {
                    crate::log_warn!(
                        "お気に入りスロット {} のホットキー解析に失敗: {} (指定値: '{}')",
                        index + 1,
                        e,
                        binding
                    );
                    continue;
                }
            };
            let id = hotkey.id();
            if used_ids.contains(&id) {
                crate::log_warn!(
                    "お気に入りスロット {} のホットキー '{}' は他の割り当てと重複するため無効",
                    index + 1,
                    binding
                );
                continue;
            }
            used_ids.insert(id);
            resolved.push((index, hotkey));
        }

        resolved
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
        fix_hotkey_field(&mut self.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr");
        self.normalize_favorite_mode_slots();
    }

    /// お気に入りスロット用ホットキー設定を正規化する
    pub(crate) fn normalize_favorite_mode_slots(&mut self) {
        if self.favorite_mode_slots.len() > consts::MAX_FAVORITE_MODES {
            self.favorite_mode_slots
                .truncate(consts::MAX_FAVORITE_MODES);
        }
        for (index, slot) in self.favorite_mode_slots.iter_mut().enumerate() {
            if slot.is_empty() {
                continue;
            }
            if parse_hotkey_binding(slot).is_err() {
                crate::log_warn!(
                    "お気に入りスロット {} のホットキー設定が無効なため無効化する (指定値: '{slot}')",
                    index + 1
                );
                slot.clear();
            }
        }
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

/// 画面 OCR のデフォルトホットキーを返す
fn default_hotkey_ocr() -> String {
    consts::DEFAULT_HOTKEY_OCR.to_string()
}

/// お気に入りスロットのデフォルトホットキー文字列を返す
fn default_favorite_slot_binding(index: usize) -> Option<String> {
    if index < 9 {
        return Some(format!("Alt+Shift+{}", index + 1));
    }
    if index < consts::MAX_FAVORITE_MODES {
        return Some(format!("Alt+Shift+F{}", index - 8));
    }
    None
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
    /// クリップボードへコピーする登録文字列
    #[serde(default)]
    pub texts: Vec<RegisteredText>,
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
            texts: Vec::new(),
            favorite_modes: Vec::new(),
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
        config.normalize_favorite_modes();
        config.normalize_pipeline();
        config.hotkeys.fix_invalid();
        (config, migration.migrated)
    }

    /// 保存前の正規化: 数値クランプとスキーマバージョンを現行へ更新
    pub fn normalize(&mut self) {
        self.clamp_values();
        self.normalize_texts();
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
        use std::collections::HashSet;

        let valid: HashSet<RefineMode> = RefineMode::iter().collect();
        self.pipeline.retain(|mode| valid.contains(mode));

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

        let valid: HashSet<RefineMode> = RefineMode::iter().collect();
        let mut seen = HashSet::new();
        self.favorite_modes.retain(|mode| {
            if !valid.contains(mode) || seen.contains(mode) {
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

// ======================================================================
// お気に入り変換モード
// ======================================================================
/// お気に入り変換モードの切り替え結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteToggleResult {
    /// お気に入りへ追加した
    Added,
    /// お気に入りから削除した
    Removed,
    /// 登録件数の上限に達している
    LimitReached,
}

/// お気に入り変換モードの移動方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteMoveDirection {
    /// 上へ移動
    Up,
    /// 下へ移動
    Down,
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

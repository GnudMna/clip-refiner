//! ホットキー設定の型と既定値

use serde::{Deserialize, Serialize};

use crate::consts;

// ======================================================================
// ホットキー設定
// ======================================================================
/// グローバルホットキーの割り当て
///
/// 各フィールドは `Alt+Shift+S` 形式の文字列で指定する
///
/// TOML は `snake_case`、設定 UI の JSON は `camelCase` でやり取りする
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeySettings {
    /// クイックセレクターの表示・非表示
    #[serde(
        default = "default_hotkey_quick_selector",
        rename(serialize = "quickSelector", deserialize = "quick_selector"),
        alias = "quickSelector",
        alias = "selector"
    )]
    pub quick_selector: String,
    /// 成功通知のON/OFF切替
    #[serde(default = "default_hotkey_notification")]
    pub notification: String,
    /// 自動変換の有効・無効切替 (v4 以前の `pause` を引き継ぐ)
    #[serde(
        default = "default_hotkey_auto_refine",
        rename(serialize = "autoRefine", deserialize = "auto_refine"),
        alias = "autoRefine",
        alias = "pause"
    )]
    pub auto_refine: String,
    /// アプリケーションの終了
    #[serde(default = "default_hotkey_quit")]
    pub quit: String,
    /// 直近の加工を取り消す
    #[serde(default = "default_hotkey_undo")]
    pub undo: String,
    /// 登録クリップセレクターの表示・非表示
    #[serde(
        default = "default_hotkey_clip_selector",
        rename(serialize = "clipSelector", deserialize = "clip_selector"),
        alias = "clipSelector"
    )]
    pub clip_selector: String,
    /// 画面範囲選択 OCR の開始
    #[serde(default = "default_hotkey_ocr")]
    pub ocr: String,
    /// 連続記録の有効・無効切替
    #[serde(
        default = "default_hotkey_continuous_record",
        rename(serialize = "continuousRecord", deserialize = "continuous_record"),
        alias = "continuousRecord"
    )]
    pub continuous_record: String,
    /// 連続記録の直近 1 件取り消し
    #[serde(
        default = "default_hotkey_continuous_record_undo",
        rename(
            serialize = "continuousRecordUndo",
            deserialize = "continuous_record_undo"
        ),
        alias = "continuousRecordUndo"
    )]
    pub continuous_record_undo: String,
    /// 連続記録キュー先頭の貼り付け
    #[serde(
        default = "default_hotkey_continuous_record_paste",
        rename(
            serialize = "continuousRecordPaste",
            deserialize = "continuous_record_paste"
        ),
        alias = "continuousRecordPaste",
        alias = "paste_queue"
    )]
    pub continuous_record_paste: String,
    /// お気に入り変換モード用ホットキー (登録順インデックスに対応。空文字で無効)
    #[serde(
        default,
        rename(serialize = "favoriteModeSlots", deserialize = "favorite_mode_slots"),
        alias = "favoriteModeSlots"
    )]
    pub favorite_mode_slots: Vec<String>,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            quick_selector: default_hotkey_quick_selector(),
            notification: default_hotkey_notification(),
            auto_refine: default_hotkey_auto_refine(),
            quit: default_hotkey_quit(),
            undo: default_hotkey_undo(),
            clip_selector: default_hotkey_clip_selector(),
            ocr: default_hotkey_ocr(),
            continuous_record: default_hotkey_continuous_record(),
            continuous_record_undo: default_hotkey_continuous_record_undo(),
            continuous_record_paste: default_hotkey_continuous_record_paste(),
            favorite_mode_slots: Vec::new(),
        }
    }
}

impl HotkeySettings {
    /// お気に入りスロットのホットキー文字列を返す
    ///
    /// 設定で空文字が指定されたスロットは `None` を返す
    ///
    /// # Arguments
    /// * `self` - 参照元のホットキー設定
    /// * `index` - お気に入りスロットのインデックス (0 始まり)
    ///
    /// # Returns
    /// * `Option<String>` - 有効なホットキー文字列。無効または範囲外の場合は `None`
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
}

// ======================================================================
// 既定値
// ======================================================================
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

/// 自動変換切替のデフォルトホットキーを返す
///
/// # Returns
/// * `String` - 自動変換切替のデフォルトホットキー
fn default_hotkey_auto_refine() -> String {
    consts::DEFAULT_HOTKEY_AUTO_REFINE.to_string()
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

/// 登録クリップセレクターのデフォルトホットキーを返す
fn default_hotkey_clip_selector() -> String {
    consts::DEFAULT_HOTKEY_CLIP_SELECTOR.to_string()
}

/// 画面 OCR のデフォルトホットキーを返す
fn default_hotkey_ocr() -> String {
    consts::DEFAULT_HOTKEY_OCR.to_string()
}

/// 連続記録切替のデフォルトホットキーを返す
fn default_hotkey_continuous_record() -> String {
    consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD.to_string()
}

/// 連続記録取り消しのデフォルトホットキーを返す
fn default_hotkey_continuous_record_undo() -> String {
    consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_UNDO.to_string()
}

/// 連続記録貼り付けのデフォルトホットキーを返す
fn default_hotkey_continuous_record_paste() -> String {
    consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_PASTE.to_string()
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

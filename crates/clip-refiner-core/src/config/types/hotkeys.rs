use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::consts;
use crate::hotkey_binding::{ParsedHotkey, parse_hotkey_binding};

use serde::{Deserialize, Serialize};

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
    /// 登録クリップセレクターの表示・非表示
    #[serde(default = "default_hotkey_clip_selector")]
    pub clip_selector: String,
    /// 画面範囲選択 OCR の開始
    #[serde(default = "default_hotkey_ocr")]
    pub ocr: String,
    /// 連続記録の有効・無効切替
    #[serde(default = "default_hotkey_continuous_record")]
    pub continuous_record: String,
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
            clip_selector: default_hotkey_clip_selector(),
            ocr: default_hotkey_ocr(),
            continuous_record: default_hotkey_continuous_record(),
            favorite_mode_slots: Vec::new(),
        }
    }
}

impl HotkeySettings {
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
            &mut self.clip_selector,
            consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
            "clip_selector",
        );
        fix_hotkey_field(&mut self.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr");
        fix_hotkey_field(
            &mut self.continuous_record,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
            "continuous_record",
        );
        self.normalize_favorite_mode_slots();
        self.fix_duplicates();
    }

    /// 重複するホットキー割り当てを解消する
    ///
    /// 固定割り当ては優先順で先勝ちとし、後続はデフォルトへ戻す。
    /// お気に入りスロットは固定割り当ておよび先に確定したスロットと重複した場合に無効化する
    pub fn fix_duplicates(&mut self) {
        let mut used: HashMap<ParsedHotkey, &'static str> = HashMap::new();

        dedupe_fixed_field(
            &mut self.quick_selector,
            "quick_selector",
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            &mut used,
        );
        dedupe_fixed_field(
            &mut self.notification,
            "notification",
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            &mut used,
        );
        dedupe_fixed_field(
            &mut self.pause,
            "pause",
            consts::DEFAULT_HOTKEY_PAUSE,
            &mut used,
        );
        dedupe_fixed_field(
            &mut self.quit,
            "quit",
            consts::DEFAULT_HOTKEY_QUIT,
            &mut used,
        );
        dedupe_fixed_field(
            &mut self.undo,
            "undo",
            consts::DEFAULT_HOTKEY_UNDO,
            &mut used,
        );
        dedupe_fixed_field(
            &mut self.clip_selector,
            "clip_selector",
            consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
            &mut used,
        );
        dedupe_fixed_field(&mut self.ocr, "ocr", consts::DEFAULT_HOTKEY_OCR, &mut used);
        dedupe_fixed_field(
            &mut self.continuous_record,
            "continuous_record",
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
            &mut used,
        );
        self.fix_duplicate_favorite_slots(&mut used);
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
                tracing::warn!(
                    "お気に入りスロット {} のホットキー設定が無効なため無効化する (指定値: '{slot}')",
                    index + 1
                );
                slot.clear();
            }
        }
    }

    /// お気に入りスロットを無効化する
    fn disable_favorite_slot(&mut self, index: usize) {
        if index >= self.favorite_mode_slots.len() {
            self.favorite_mode_slots.resize(index + 1, String::new());
        }
        self.favorite_mode_slots[index].clear();
    }

    /// お気に入りスロットの重複を無効化する
    fn fix_duplicate_favorite_slots(&mut self, used: &mut HashMap<ParsedHotkey, &'static str>) {
        for index in 0..consts::MAX_FAVORITE_MODES {
            let Some(binding) = self.favorite_slot_binding(index) else {
                continue;
            };
            let Ok(parsed) = parse_hotkey_binding(&binding) else {
                continue;
            };
            if let Entry::Vacant(entry) = used.entry(parsed) {
                entry.insert("favorite_mode_slot");
            } else {
                tracing::warn!(
                    "お気に入りスロット {} のホットキー '{binding}' は他の割り当てと重複するため無効化する",
                    index + 1
                );
                self.disable_favorite_slot(index);
            }
        }
    }
}

/// 固定ホットキー割り当ての重複を解消する
fn dedupe_fixed_field(
    field: &mut String,
    label: &'static str,
    default: &str,
    used: &mut HashMap<ParsedHotkey, &'static str>,
) {
    let original = field.clone();
    let mut candidate = original.clone();

    loop {
        let Ok(parsed) = parse_hotkey_binding(&candidate) else {
            return;
        };
        if let Entry::Vacant(entry) = used.entry(parsed) {
            *field = candidate;
            entry.insert(label);
            return;
        }
        if candidate == default {
            tracing::warn!("ホットキー設定 '{label}' の重複を解消できない (指定値: '{original}')");
            return;
        }
        if let Some(first) = used.get(&parsed) {
            tracing::warn!(
                "ホットキー設定 '{label}' が '{first}' と重複するためデフォルト '{default}' に置き換える (指定値: '{original}')"
            );
        }
        candidate = default.to_string();
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
        tracing::warn!(
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

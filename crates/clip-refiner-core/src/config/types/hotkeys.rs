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
            pause: default_hotkey_pause(),
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

// ======================================================================
// 正規化結果
// ======================================================================
/// ホットキー正規化で行った修正の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyFixKind {
    /// 不正な割り当てをデフォルトへ置き換えた
    InvalidReplacedWithDefault,
    /// 重複する割り当てをデフォルトへ置き換えた
    DuplicateReplacedWithDefault,
    /// デフォルト同士の重複を解消できなかった
    DuplicateUnresolved,
    /// 不正なお気に入りスロットを無効化した
    FavoriteInvalidDisabled,
    /// 重複するお気に入りスロットを無効化した
    FavoriteDuplicateDisabled,
}

/// ホットキー正規化で行った 1 件の修正
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyFix {
    /// 設定フィールドの内部キー (`quick_selector` など)
    pub field: String,
    /// 修正の種類
    pub kind: HotkeyFixKind,
    /// ユーザーが指定した元の値
    pub original: String,
    /// 置き換え後の値 (無効化時は空)
    pub replacement: String,
    /// 重複相手のフィールドキー (重複系以外は空)
    pub conflicted_with: String,
}

impl HotkeyFix {
    /// 設定 UI 向けの 1 行メッセージを返す
    ///
    /// # Arguments
    /// * `self` - 正規化で記録した修正
    ///
    /// # Returns
    /// * `String` - ユーザー向け説明
    pub fn user_message(&self) -> String {
        let label = hotkey_field_display_name(&self.field);
        match self.kind {
            HotkeyFixKind::InvalidReplacedWithDefault => {
                format!(
                    "「{label}」のホットキーが無効なためデフォルト ({}) に戻しました",
                    self.replacement
                )
            }
            HotkeyFixKind::DuplicateReplacedWithDefault => {
                let other = hotkey_field_display_name(&self.conflicted_with);
                format!(
                    "「{label}」のホットキーが「{other}」と重複するためデフォルト ({}) に戻しました",
                    self.replacement
                )
            }
            HotkeyFixKind::DuplicateUnresolved => {
                format!(
                    "「{label}」のホットキー重複を解消できませんでした (指定値: {})",
                    self.original
                )
            }
            HotkeyFixKind::FavoriteInvalidDisabled => {
                format!("「{label}」のホットキーが無効なため無効化しました")
            }
            HotkeyFixKind::FavoriteDuplicateDisabled => {
                format!("「{label}」のホットキーが他の割り当てと重複するため無効化しました")
            }
        }
    }
}

/// 正規化結果を設定 UI 向けのフィードバック文言へまとめる
///
/// # Arguments
/// * `fixes` - `fix_invalid` が返した修正一覧
///
/// # Returns
/// * `Option<String>` - 修正が 1 件以上ある場合のみメッセージ
pub fn format_hotkey_fixes_message(fixes: &[HotkeyFix]) -> Option<String> {
    match fixes {
        [] => None,
        [only] => Some(only.user_message()),
        many => {
            let labels = many
                .iter()
                .map(|fix| format!("「{}」", hotkey_field_display_name(&fix.field)))
                .collect::<Vec<_>>()
                .join("、");
            Some(format!("ホットキーを正規化しました: {labels}"))
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

    /// 不正なホットキー文字列をデフォルト値へ置き換える
    ///
    /// 重複解消もあわせて実行する
    ///
    /// # Arguments
    /// * `self` - 修正対象のホットキー設定
    ///
    /// # Returns
    /// * `Vec<HotkeyFix>` - 実施した修正の一覧 (修正なしは空)
    pub fn fix_invalid(&mut self) -> Vec<HotkeyFix> {
        let mut fixes = Vec::new();
        fix_hotkey_field(
            &mut self.quick_selector,
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            "quick_selector",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.notification,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            "notification",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.pause,
            consts::DEFAULT_HOTKEY_PAUSE,
            "pause",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.quit,
            consts::DEFAULT_HOTKEY_QUIT,
            "quit",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.undo,
            consts::DEFAULT_HOTKEY_UNDO,
            "undo",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.clip_selector,
            consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
            "clip_selector",
            &mut fixes,
        );
        fix_hotkey_field(&mut self.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr", &mut fixes);
        fix_hotkey_field(
            &mut self.continuous_record,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
            "continuous_record",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.continuous_record_undo,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_UNDO,
            "continuous_record_undo",
            &mut fixes,
        );
        fix_hotkey_field(
            &mut self.continuous_record_paste,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_PASTE,
            "continuous_record_paste",
            &mut fixes,
        );
        self.normalize_favorite_mode_slots(&mut fixes);
        fixes.extend(self.fix_duplicates());
        fixes
    }

    /// 重複するホットキー割り当てを解消する
    ///
    /// 固定割り当ては優先順で先勝ちとし、後続はデフォルトへ戻す
    /// お気に入りスロットは固定割り当ておよび先に確定したスロットと重複した場合に無効化する
    ///
    /// # Arguments
    /// * `self` - 修正対象のホットキー設定
    ///
    /// # Returns
    /// * `Vec<HotkeyFix>` - 実施した修正の一覧 (修正なしは空)
    pub fn fix_duplicates(&mut self) -> Vec<HotkeyFix> {
        let mut fixes = Vec::new();
        let mut used: HashMap<ParsedHotkey, &'static str> = HashMap::new();

        dedupe_fixed_field(
            &mut self.quick_selector,
            "quick_selector",
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.notification,
            "notification",
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.pause,
            "pause",
            consts::DEFAULT_HOTKEY_PAUSE,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.quit,
            "quit",
            consts::DEFAULT_HOTKEY_QUIT,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.undo,
            "undo",
            consts::DEFAULT_HOTKEY_UNDO,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.clip_selector,
            "clip_selector",
            consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.ocr,
            "ocr",
            consts::DEFAULT_HOTKEY_OCR,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.continuous_record,
            "continuous_record",
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.continuous_record_undo,
            "continuous_record_undo",
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_UNDO,
            &mut used,
            &mut fixes,
        );
        dedupe_fixed_field(
            &mut self.continuous_record_paste,
            "continuous_record_paste",
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_PASTE,
            &mut used,
            &mut fixes,
        );
        self.fix_duplicate_favorite_slots(&mut used, &mut fixes);
        fixes
    }

    /// お気に入りスロット用ホットキー設定を正規化する
    pub(crate) fn normalize_favorite_mode_slots(&mut self, fixes: &mut Vec<HotkeyFix>) {
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
                let original = slot.clone();
                slot.clear();
                fixes.push(HotkeyFix {
                    field: favorite_slot_field_key(index),
                    kind: HotkeyFixKind::FavoriteInvalidDisabled,
                    original,
                    replacement: String::new(),
                    conflicted_with: String::new(),
                });
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
    fn fix_duplicate_favorite_slots(
        &mut self,
        used: &mut HashMap<ParsedHotkey, &'static str>,
        fixes: &mut Vec<HotkeyFix>,
    ) {
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
                fixes.push(HotkeyFix {
                    field: favorite_slot_field_key(index),
                    kind: HotkeyFixKind::FavoriteDuplicateDisabled,
                    original: binding,
                    replacement: String::new(),
                    conflicted_with: String::new(),
                });
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
    fixes: &mut Vec<HotkeyFix>,
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
            fixes.push(HotkeyFix {
                field: label.to_string(),
                kind: HotkeyFixKind::DuplicateUnresolved,
                original,
                replacement: candidate,
                conflicted_with: String::new(),
            });
            return;
        }
        let conflicted_with = used.get(&parsed).copied().unwrap_or("unknown").to_string();
        tracing::warn!(
            "ホットキー設定 '{label}' が '{conflicted_with}' と重複するためデフォルト '{default}' に置き換える (指定値: '{original}')"
        );
        fixes.push(HotkeyFix {
            field: label.to_string(),
            kind: HotkeyFixKind::DuplicateReplacedWithDefault,
            original: original.clone(),
            replacement: default.to_string(),
            conflicted_with,
        });
        candidate = default.to_string();
    }
}

/// 不正なホットキー文字列をデフォルト値へ置き換える
///
/// # Arguments
/// * `field` - 不正なホットキー文字列
/// * `default` - デフォルトホットキー文字列
/// * `label` - ホットキー設定のラベル
/// * `fixes` - 実施した修正の追記先
fn fix_hotkey_field(field: &mut String, default: &str, label: &str, fixes: &mut Vec<HotkeyFix>) {
    if parse_hotkey_binding(field).is_err() {
        tracing::warn!(
            "ホットキー設定 '{label}' が無効なためデフォルト '{default}' に置き換える (指定値: '{field}')"
        );
        let original = field.clone();
        *field = default.to_string();
        fixes.push(HotkeyFix {
            field: label.to_string(),
            kind: HotkeyFixKind::InvalidReplacedWithDefault,
            original,
            replacement: default.to_string(),
            conflicted_with: String::new(),
        });
    }
}

/// ホットキー設定フィールドの表示名を返す
fn hotkey_field_display_name(field: &str) -> String {
    if let Some(index) = field
        .strip_prefix("favorite_mode_slot_")
        .and_then(|raw| raw.parse::<usize>().ok())
    {
        return format!("お気に入り {}", index + 1);
    }
    match field {
        "quick_selector" => "クイックセレクター".to_string(),
        "notification" => "成功通知の切替".to_string(),
        "pause" => "一時停止 / 再開".to_string(),
        "quit" => "終了".to_string(),
        "undo" => "加工の取り消し".to_string(),
        "clip_selector" => "登録クリップセレクター".to_string(),
        "ocr" => "画面 OCR".to_string(),
        "continuous_record" => "連続記録の切替".to_string(),
        "continuous_record_undo" => "連続記録の取り消し".to_string(),
        "continuous_record_paste" => "連続記録の貼り付け".to_string(),
        "favorite_mode_slot" => "お気に入り".to_string(),
        other => other.to_string(),
    }
}

/// お気に入りスロットの内部キーを返す
fn favorite_slot_field_key(index: usize) -> String {
    format!("favorite_mode_slot_{index}")
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 不正な固定ホットキーの修正メッセージを組み立てられること
    #[test]
    fn format_invalid_fix_message() {
        let mut hotkeys = HotkeySettings {
            pause: "Bad+Key".to_string(),
            ..HotkeySettings::default()
        };
        let fixes = hotkeys.fix_invalid();
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].kind, HotkeyFixKind::InvalidReplacedWithDefault);
        let message = format_hotkey_fixes_message(&fixes).expect("メッセージ");
        assert!(message.contains("一時停止 / 再開"));
        assert!(message.contains(consts::DEFAULT_HOTKEY_PAUSE));
    }

    /// 重複する固定ホットキーの修正メッセージを組み立てられること
    #[test]
    fn format_duplicate_fix_message() {
        let mut hotkeys = HotkeySettings {
            notification: "Ctrl+Shift+N".to_string(),
            pause: "Ctrl+Shift+N".to_string(),
            ..HotkeySettings::default()
        };
        let fixes = hotkeys.fix_duplicates();
        let pause_fix = fixes
            .iter()
            .find(|fix| fix.field == "pause")
            .expect("pause の重複修正");
        assert_eq!(pause_fix.kind, HotkeyFixKind::DuplicateReplacedWithDefault);
        let message =
            format_hotkey_fixes_message(std::slice::from_ref(pause_fix)).expect("メッセージ");
        assert!(message.contains("一時停止 / 再開"));
        assert!(message.contains("成功通知の切替"));
        assert_eq!(hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE);
    }

    /// 重複するお気に入りスロットの無効化メッセージを組み立てられること
    #[test]
    fn format_favorite_duplicate_disabled_message() {
        let mut hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Ctrl+Alt+F10".to_string(), "Ctrl+Alt+F10".to_string()],
            ..HotkeySettings::default()
        };
        let fixes = hotkeys.fix_duplicates();
        let slot_fix = fixes
            .iter()
            .find(|fix| fix.field == "favorite_mode_slot_1")
            .expect("お気に入り 2 の重複無効化");
        assert_eq!(slot_fix.kind, HotkeyFixKind::FavoriteDuplicateDisabled);
        let message =
            format_hotkey_fixes_message(std::slice::from_ref(slot_fix)).expect("メッセージ");
        assert!(message.contains("お気に入り 2"));
        assert!(message.contains("無効化"));
        assert!(hotkeys.favorite_mode_slots[1].is_empty());
    }
}

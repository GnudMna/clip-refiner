//! ホットキー文字列の不正・重複補正

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use super::fix::{HotkeyFix, HotkeyFixKind, favorite_slot_field_key};
use super::settings::HotkeySettings;

use crate::consts;
use crate::hotkey_binding::{ParsedHotkey, parse_hotkey_binding};

impl HotkeySettings {
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

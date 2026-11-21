use global_hotkey::hotkey::HotKey;
use serde::Serialize;

use super::convert::parse_global_hotkey;
use crate::config::HotkeySettings;
use crate::consts;
use crate::refiner::RefineMode;

// ======================================================================
// ショートカット一覧
// ======================================================================
/// ショートカット一覧の1行分
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutEntry {
    /// ホットキー文字列 (`Alt+Shift+S` 形式)
    pub binding: String,
    /// 動作の説明
    pub label: String,
}

/// ショートカット一覧を構造化データとして生成する
pub(crate) fn shortcut_list_entries(
    hotkeys: &HotkeySettings,
    favorite_modes: &[RefineMode],
) -> Vec<ShortcutEntry> {
    let mut entries = vec![
        ShortcutEntry {
            binding: hotkeys.quick_selector.clone(),
            label: "クイックセレクター".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.clip_selector.clone(),
            label: "登録クリップセレクター".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.ocr.clone(),
            label: "画面 OCR".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.notification.clone(),
            label: "成功通知の切替".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.pause.clone(),
            label: "一時停止 / 再開".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.continuous_record.clone(),
            label: "連続記録の切替".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.continuous_record_undo.clone(),
            label: "連続記録の取り消し".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.continuous_record_paste.clone(),
            label: "連続記録の貼り付け".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.undo.clone(),
            label: "加工の取り消し".to_string(),
        },
        ShortcutEntry {
            binding: hotkeys.quit.clone(),
            label: "終了".to_string(),
        },
    ];
    for (index, mode) in favorite_modes.iter().enumerate() {
        if let Some(binding) = hotkeys.favorite_slot_binding(index) {
            entries.push(ShortcutEntry {
                binding,
                label: format!("{} (お気に入り)", mode.label()),
            });
        }
    }
    entries
}

/// お気に入りスロット用ホットキーを解決する
///
/// 固定ホットキーおよび先に割り当てたスロットと重複する割り当ては除外する
pub(crate) fn resolve_favorite_slot_hotkeys(
    hotkeys: &HotkeySettings,
    favorite_count: usize,
    reserved: &[HotKey],
) -> Vec<(usize, HotKey)> {
    use std::collections::HashSet;

    let mut used_ids: HashSet<u32> = reserved.iter().map(HotKey::id).collect();
    let mut resolved = Vec::new();

    for index in 0..favorite_count.min(consts::MAX_FAVORITE_MODES) {
        let Some(binding) = hotkeys.favorite_slot_binding(index) else {
            continue;
        };
        let hotkey = match parse_global_hotkey(&binding) {
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 固定ホットキーと重複するお気に入りスロットは除外されるようにすること
    #[test]
    fn resolve_favorite_slot_skips_duplicate_fixed_hotkeys() {
        let hotkeys = HotkeySettings {
            quick_selector: "Alt+Shift+1".to_string(),
            favorite_mode_slots: vec!["Alt+Shift+1".to_string()],
            ..HotkeySettings::default()
        };
        let reserved = vec![parse_global_hotkey("Alt+Shift+1").expect("parse")];

        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 1, &reserved);

        assert!(resolved.is_empty());
    }

    /// 先に割り当てたスロットと重複するお気に入りスロットは除外されるようにすること
    #[test]
    fn resolve_favorite_slot_skips_duplicate_among_slots() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Alt+Shift+F10".to_string(), "Alt+Shift+F10".to_string()],
            ..HotkeySettings::default()
        };

        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 2, &[]);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, 0);
    }

    /// 空文字のスロット設定は無効としてスキップされるようにすること
    #[test]
    fn resolve_favorite_slot_skips_empty_binding() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec![String::new()],
            ..HotkeySettings::default()
        };

        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 1, &[]);

        assert!(resolved.is_empty());
    }

    /// 重複するお気に入りホットキーは除外されるようにすること
    #[test]
    fn resolve_favorite_slot_hotkeys_skips_duplicates() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Alt+Shift+S".to_string()],
            ..HotkeySettings::default()
        };
        let reserved =
            vec![parse_global_hotkey(consts::DEFAULT_HOTKEY_QUICK_SELECTOR).expect("解析に失敗")];
        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 2, &reserved);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, 1);
    }

    /// お気に入り登録後のショートカット一覧にスロットが反映されるようにすること
    #[test]
    fn shortcut_list_includes_registered_favorite_mode() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Alt+Shift+F1".to_string()],
            ..HotkeySettings::default()
        };
        let favorites = vec![RefineMode::JsonFormat];

        let entries = shortcut_list_entries(&hotkeys, &favorites);

        assert!(entries.iter().any(|entry| {
            entry.binding == "Alt+Shift+F1"
                && entry.label.contains("JSON")
                && entry.label.contains("お気に入り")
        }));
    }
}

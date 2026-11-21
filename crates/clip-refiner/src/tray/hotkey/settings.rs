use crate::config::HotkeySettings;
use crate::consts;
use crate::hotkey_binding::parse_hotkey_binding;
use crate::platform;
use crate::refiner::RefineMode;

use global_hotkey::hotkey::HotKey;

// ======================================================================
// HotkeySettings 拡張 (アプリ UI 向け)
// ======================================================================
/// ショートカット一覧表示用の文字列を生成する
pub(crate) fn shortcut_list_text(
    hotkeys: &HotkeySettings,
    favorite_modes: &[RefineMode],
) -> String {
    let mut lines = vec![
        format!("{}: クイックセレクター", hotkeys.quick_selector),
        format!("{}: 登録クリップセレクター", hotkeys.clip_selector),
    ];
    if platform::supports_screen_ocr() {
        lines.push(format!("{}: 画面 OCR", hotkeys.ocr));
    }
    lines.extend([
        format!("{}: 成功通知の切替", hotkeys.notification),
        format!("{}: 一時停止/再開", hotkeys.pause),
        format!("{}: 加工の取り消し", hotkeys.undo),
        format!("{}: 終了", hotkeys.quit),
    ]);
    for (index, mode) in favorite_modes.iter().enumerate() {
        if let Some(binding) = hotkeys.favorite_slot_binding(index) {
            lines.push(format!("{binding}: {} (お気に入り)", mode.label()));
        }
    }
    lines.join("\n")
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 固定ホットキーと重複するお気に入りスロットは除外されること
    #[test]
    fn resolve_favorite_slot_skips_duplicate_fixed_hotkeys() {
        let hotkeys = HotkeySettings {
            quick_selector: "Alt+Shift+1".to_string(),
            favorite_mode_slots: vec!["Alt+Shift+1".to_string()],
            ..HotkeySettings::default()
        };
        let reserved = vec![parse_hotkey_binding("Alt+Shift+1").expect("parse")];

        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 1, &reserved);

        assert!(resolved.is_empty());
    }

    /// 先に割り当てたスロットと重複するお気に入りスロットは除外されること
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

    /// 空文字のスロット設定は無効としてスキップされること
    #[test]
    fn resolve_favorite_slot_skips_empty_binding() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec![String::new()],
            ..HotkeySettings::default()
        };

        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 1, &[]);

        assert!(resolved.is_empty());
    }

    /// 重複するお気に入りホットキーは除外されること
    #[test]
    fn resolve_favorite_slot_hotkeys_skips_duplicates() {
        let hotkeys = HotkeySettings {
            favorite_mode_slots: vec!["Alt+Shift+S".to_string()],
            ..HotkeySettings::default()
        };
        let reserved =
            vec![parse_hotkey_binding(consts::DEFAULT_HOTKEY_QUICK_SELECTOR).expect("解析に失敗")];
        let resolved = resolve_favorite_slot_hotkeys(&hotkeys, 2, &reserved);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, 1);
    }
}

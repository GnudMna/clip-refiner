//! ホットキー正規化の修正結果と表示メッセージ

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
    /// 重複相手のフィールドキー (`DuplicateReplacedWithDefault` 以外は空)
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
/// * `fixes` - `fix_invalid` / `fix_duplicates` が返した修正一覧
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
        "auto_refine" => "自動変換の切替".to_string(),
        "quit" => "終了".to_string(),
        "undo" => "加工の取り消し".to_string(),
        "clip_selector" => "登録クリップセレクター".to_string(),
        "ocr" => "画面 OCR".to_string(),
        "continuous_record" => "連続記録モードの切替".to_string(),
        "continuous_record_undo" => "連続記録モードの記録取り消し".to_string(),
        "continuous_record_paste" => "連続記録モードの記録貼り付け".to_string(),
        "favorite_mode_slot" => "お気に入り".to_string(),
        other => other.to_string(),
    }
}

/// お気に入りスロットの内部キーを返す
pub(super) fn favorite_slot_field_key(index: usize) -> String {
    format!("favorite_mode_slot_{index}")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::HotkeySettings;
    use super::{HotkeyFixKind, format_hotkey_fixes_message};
    use crate::consts;

    /// 不正な固定ホットキーの修正メッセージを組み立てられるようにすること
    #[test]
    fn format_invalid_fix_message() {
        let mut hotkeys = HotkeySettings {
            auto_refine: "Bad+Key".to_string(),
            ..HotkeySettings::default()
        };
        let fixes = hotkeys.fix_invalid();
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].kind, HotkeyFixKind::InvalidReplacedWithDefault);
        let message = format_hotkey_fixes_message(&fixes).expect("メッセージ");
        assert!(message.contains("自動変換の切替"));
        assert!(message.contains(consts::DEFAULT_HOTKEY_AUTO_REFINE));
    }

    /// 重複する固定ホットキーの修正メッセージを組み立てられるようにすること
    #[test]
    fn format_duplicate_fix_message() {
        let mut hotkeys = HotkeySettings {
            notification: "Ctrl+Shift+N".to_string(),
            auto_refine: "Ctrl+Shift+N".to_string(),
            ..HotkeySettings::default()
        };
        let fixes = hotkeys.fix_duplicates();
        let auto_refine_fix = fixes
            .iter()
            .find(|fix| fix.field == "auto_refine")
            .expect("auto_refine の重複修正");
        assert_eq!(
            auto_refine_fix.kind,
            HotkeyFixKind::DuplicateReplacedWithDefault
        );
        let message =
            format_hotkey_fixes_message(std::slice::from_ref(auto_refine_fix)).expect("メッセージ");
        assert!(message.contains("自動変換の切替"));
        assert!(message.contains("成功通知の切替"));
        assert_eq!(hotkeys.auto_refine, consts::DEFAULT_HOTKEY_AUTO_REFINE);
    }

    /// 重複するお気に入りスロットの無効化メッセージを組み立てられるようにすること
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

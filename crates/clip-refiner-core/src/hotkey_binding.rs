// ======================================================================
// ホットキー型
// ======================================================================
/// 修飾キーの組み合わせ (`Alt+Shift+S` 形式の修飾部)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "修飾キーごとの独立したフラグで、状態機械にまとめる意味がない"
)]
pub struct HotkeyModifiers {
    /// Alt 修飾
    pub alt: bool,
    /// Shift 修飾
    pub shift: bool,
    /// Control 修飾
    pub control: bool,
    /// Meta / Super / Win 修飾
    pub meta: bool,
}

/// ホットキーのキー本体 (修飾キーを除く)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyKey {
    /// A〜Z
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    /// 0〜9
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    /// F1〜F12
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    /// F13〜F24
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    /// Backspace
    Backspace,
    /// Space
    Space,
    /// Enter
    Enter,
    /// Tab
    Tab,
    /// Escape
    Escape,
    /// Insert
    Insert,
    /// Delete
    Delete,
    /// Home
    Home,
    /// End
    End,
    /// Page Up
    PageUp,
    /// Page Down
    PageDown,
    /// 矢印キー
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    /// Print Screen
    PrintScreen,
    /// Pause / Break
    Pause,
    /// Scroll Lock
    ScrollLock,
    /// テンキー 0〜9
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    /// テンキー `+` / `-` / `*` / `/` / `.`
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    /// 記号キー `-` / `,` / `.` / `/` (US / JIS 配列で同じ仮想キー)
    Minus,
    Comma,
    Period,
    Slash,
}

/// `Alt+Shift+S` 形式から解析したホットキー
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsedHotkey {
    /// 修飾キー
    pub modifiers: HotkeyModifiers,
    /// キー本体
    pub key: HotkeyKey,
}

// ======================================================================
// エラー型
// ======================================================================
/// ホットキー文字列の解析失敗理由
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyParseError {
    /// 空文字列
    #[error("空のホットキー指定")]
    Empty,
    /// キー本体が指定されていない
    #[error("キーが指定されていません")]
    MissingKey,
    /// 未対応のキー
    #[error("未対応のキー: {0}")]
    UnsupportedKey(String),
    /// 未対応の修飾キー
    #[error("未対応の修飾キー: {0}")]
    UnsupportedModifier(String),
}

/// 全デフォルト文字列解析が失敗した場合の最終フォールバック (`Alt+Shift+P`)
const LAST_RESORT_HOTKEY: ParsedHotkey = ParsedHotkey {
    modifiers: HotkeyModifiers {
        alt: true,
        shift: true,
        control: false,
        meta: false,
    },
    key: HotkeyKey::KeyP,
};

// ======================================================================
// ホットキー解析
// ======================================================================
/// `Alt+Shift+S` 形式の文字列を `ParsedHotkey` に変換する
///
/// 修飾キーは `Alt` / `Shift` / `Ctrl` / `Control` / `Meta` / `Super` / `Win` に対応する
/// キーは `A`〜`Z` / `0`〜`9` / `F1`〜`F24` / 編集・移動キー / テンキー / 一部の記号キーに対応する
/// (一覧は `docs/user/configuration.md` のホットキー形式)
///
/// # Arguments
/// * `binding` - ホットキー文字列
///
/// # Returns
/// * `Ok(ParsedHotkey)` - 解析に成功したホットキー
///
/// # Errors
/// 空文字列、未対応の修飾キー、未対応のキーのいずれか
pub fn parse_hotkey_binding(binding: &str) -> Result<ParsedHotkey, HotkeyParseError> {
    let parts: Vec<&str> = binding
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(HotkeyParseError::Empty);
    }

    let key_name = parts.last().ok_or(HotkeyParseError::MissingKey)?;
    let key = parse_key_name(key_name)
        .ok_or_else(|| HotkeyParseError::UnsupportedKey(key_name.to_string()))?;

    let mut modifiers = HotkeyModifiers::default();
    for part in &parts[..parts.len() - 1] {
        apply_modifier(part, &mut modifiers)?;
    }

    Ok(ParsedHotkey { modifiers, key })
}

/// 設定値を解析し、失敗時はデフォルトへフォールバックする
///
/// 指定値とデフォルト値の両方が不正な場合は `Alt+Shift+P` 相当の最終フォールバックを返す
///
/// # Arguments
/// * `binding` - 設定ファイルのホットキー文字列
/// * `default_binding` - フォールバック先のデフォルト文字列
///
/// # Returns
/// * `ParsedHotkey` - 解析済みホットキー
#[must_use]
pub fn parse_hotkey_or_default(binding: &str, default_binding: &str) -> ParsedHotkey {
    parse_hotkey_with_fallback(binding, default_binding, None)
}

/// 設定値を解析し、失敗時はデフォルトへフォールバックする
///
/// `label` はログ出力用の設定項目名
///
/// # Arguments
/// * `binding` - 設定ファイルのホットキー文字列
/// * `default_binding` - フォールバック先のデフォルト文字列
/// * `label` - ログ出力用の設定項目名
///
/// # Returns
/// * `ParsedHotkey` - 解析済みホットキー
#[must_use]
pub fn parse_hotkey_or_default_labeled(
    binding: &str,
    default_binding: &str,
    label: &str,
) -> ParsedHotkey {
    parse_hotkey_with_fallback(binding, default_binding, Some(label))
}

/// 指定値を解析し、失敗時はデフォルト、それも失敗時は最終フォールバックを返す
fn parse_hotkey_with_fallback(
    binding: &str,
    default_binding: &str,
    label: Option<&str>,
) -> ParsedHotkey {
    match parse_hotkey_binding(binding) {
        Ok(parsed) => parsed,
        Err(error) => {
            if let Some(label) = label {
                tracing::warn!(
                    hotkey.label = label,
                    hotkey.binding = binding,
                    %error,
                    hotkey.fallback = default_binding,
                    "ホットキー設定の解析に失敗、デフォルトへフォールバック"
                );
            }
            match parse_hotkey_binding(default_binding) {
                Ok(parsed) => parsed,
                Err(error) => {
                    tracing::error!(
                        hotkey.fallback = default_binding,
                        %error,
                        "デフォルトホットキーの解析に失敗、最終フォールバックを使用"
                    );
                    LAST_RESORT_HOTKEY
                }
            }
        }
    }
}

/// 修飾キー名を修飾キー構造体へ反映する
fn apply_modifier(name: &str, modifiers: &mut HotkeyModifiers) -> Result<(), HotkeyParseError> {
    match name.to_lowercase().as_str() {
        "alt" => modifiers.alt = true,
        "shift" => modifiers.shift = true,
        "ctrl" | "control" => modifiers.control = true,
        "meta" | "super" | "win" => modifiers.meta = true,
        other => return Err(HotkeyParseError::UnsupportedModifier(other.to_string())),
    }
    Ok(())
}

/// キー名を `HotkeyKey` に変換する
fn parse_key_name(name: &str) -> Option<HotkeyKey> {
    let upper = name.to_uppercase();
    if upper.len() == 1 {
        let ch = upper.chars().next()?;
        if ch.is_ascii_digit() {
            return digit_to_key(ch);
        }
        if ch.is_ascii_alphabetic() {
            return letter_to_key(ch);
        }
        return symbol_to_key(ch);
    }

    if let Some(num) = upper.strip_prefix("NUMPAD") {
        return numpad_to_key(num);
    }

    if let Some(num) = upper
        .strip_prefix('F')
        .filter(|num| num.bytes().all(|b| b.is_ascii_digit()))
    {
        return match num.parse::<u8>().ok()? {
            1 => Some(HotkeyKey::F1),
            2 => Some(HotkeyKey::F2),
            3 => Some(HotkeyKey::F3),
            4 => Some(HotkeyKey::F4),
            5 => Some(HotkeyKey::F5),
            6 => Some(HotkeyKey::F6),
            7 => Some(HotkeyKey::F7),
            8 => Some(HotkeyKey::F8),
            9 => Some(HotkeyKey::F9),
            10 => Some(HotkeyKey::F10),
            11 => Some(HotkeyKey::F11),
            12 => Some(HotkeyKey::F12),
            13 => Some(HotkeyKey::F13),
            14 => Some(HotkeyKey::F14),
            15 => Some(HotkeyKey::F15),
            16 => Some(HotkeyKey::F16),
            17 => Some(HotkeyKey::F17),
            18 => Some(HotkeyKey::F18),
            19 => Some(HotkeyKey::F19),
            20 => Some(HotkeyKey::F20),
            21 => Some(HotkeyKey::F21),
            22 => Some(HotkeyKey::F22),
            23 => Some(HotkeyKey::F23),
            24 => Some(HotkeyKey::F24),
            _ => None,
        };
    }

    Some(match upper.as_str() {
        "BACKSPACE" => HotkeyKey::Backspace,
        "SPACE" => HotkeyKey::Space,
        "ENTER" | "RETURN" => HotkeyKey::Enter,
        "TAB" => HotkeyKey::Tab,
        "ESCAPE" | "ESC" => HotkeyKey::Escape,
        "INSERT" | "INS" => HotkeyKey::Insert,
        "DELETE" | "DEL" => HotkeyKey::Delete,
        "HOME" => HotkeyKey::Home,
        "END" => HotkeyKey::End,
        "PAGEUP" | "PGUP" => HotkeyKey::PageUp,
        "PAGEDOWN" | "PGDN" => HotkeyKey::PageDown,
        "ARROWUP" | "UP" => HotkeyKey::ArrowUp,
        "ARROWDOWN" | "DOWN" => HotkeyKey::ArrowDown,
        "ARROWLEFT" | "LEFT" => HotkeyKey::ArrowLeft,
        "ARROWRIGHT" | "RIGHT" => HotkeyKey::ArrowRight,
        "PRINTSCREEN" => HotkeyKey::PrintScreen,
        "PAUSE" => HotkeyKey::Pause,
        "SCROLLLOCK" => HotkeyKey::ScrollLock,
        "MINUS" => HotkeyKey::Minus,
        "COMMA" => HotkeyKey::Comma,
        "PERIOD" => HotkeyKey::Period,
        "SLASH" => HotkeyKey::Slash,
        _ => return None,
    })
}

/// 記号 1 文字を `HotkeyKey` に変換する
///
/// `+` は区切り文字のため扱わない
fn symbol_to_key(ch: char) -> Option<HotkeyKey> {
    Some(match ch {
        '-' => HotkeyKey::Minus,
        ',' => HotkeyKey::Comma,
        '.' => HotkeyKey::Period,
        '/' => HotkeyKey::Slash,
        _ => return None,
    })
}

/// `Numpad` 以降の名前をテンキーの `HotkeyKey` に変換する
fn numpad_to_key(suffix: &str) -> Option<HotkeyKey> {
    Some(match suffix {
        "0" => HotkeyKey::Numpad0,
        "1" => HotkeyKey::Numpad1,
        "2" => HotkeyKey::Numpad2,
        "3" => HotkeyKey::Numpad3,
        "4" => HotkeyKey::Numpad4,
        "5" => HotkeyKey::Numpad5,
        "6" => HotkeyKey::Numpad6,
        "7" => HotkeyKey::Numpad7,
        "8" => HotkeyKey::Numpad8,
        "9" => HotkeyKey::Numpad9,
        "ADD" => HotkeyKey::NumpadAdd,
        "SUBTRACT" => HotkeyKey::NumpadSubtract,
        "MULTIPLY" => HotkeyKey::NumpadMultiply,
        "DIVIDE" => HotkeyKey::NumpadDivide,
        "DECIMAL" => HotkeyKey::NumpadDecimal,
        _ => return None,
    })
}

/// 数字を `HotkeyKey` に変換する
fn digit_to_key(ch: char) -> Option<HotkeyKey> {
    Some(match ch {
        '0' => HotkeyKey::Digit0,
        '1' => HotkeyKey::Digit1,
        '2' => HotkeyKey::Digit2,
        '3' => HotkeyKey::Digit3,
        '4' => HotkeyKey::Digit4,
        '5' => HotkeyKey::Digit5,
        '6' => HotkeyKey::Digit6,
        '7' => HotkeyKey::Digit7,
        '8' => HotkeyKey::Digit8,
        '9' => HotkeyKey::Digit9,
        _ => return None,
    })
}

/// 文字を `HotkeyKey` に変換する
fn letter_to_key(ch: char) -> Option<HotkeyKey> {
    Some(match ch {
        'A' => HotkeyKey::KeyA,
        'B' => HotkeyKey::KeyB,
        'C' => HotkeyKey::KeyC,
        'D' => HotkeyKey::KeyD,
        'E' => HotkeyKey::KeyE,
        'F' => HotkeyKey::KeyF,
        'G' => HotkeyKey::KeyG,
        'H' => HotkeyKey::KeyH,
        'I' => HotkeyKey::KeyI,
        'J' => HotkeyKey::KeyJ,
        'K' => HotkeyKey::KeyK,
        'L' => HotkeyKey::KeyL,
        'M' => HotkeyKey::KeyM,
        'N' => HotkeyKey::KeyN,
        'O' => HotkeyKey::KeyO,
        'P' => HotkeyKey::KeyP,
        'Q' => HotkeyKey::KeyQ,
        'R' => HotkeyKey::KeyR,
        'S' => HotkeyKey::KeyS,
        'T' => HotkeyKey::KeyT,
        'U' => HotkeyKey::KeyU,
        'V' => HotkeyKey::KeyV,
        'W' => HotkeyKey::KeyW,
        'X' => HotkeyKey::KeyX,
        'Y' => HotkeyKey::KeyY,
        'Z' => HotkeyKey::KeyZ,
        _ => return None,
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts;

    /// デフォルトホットキー文字列が解析できるようにすること
    #[test]
    fn test_parse_default_hotkeys() {
        for binding in [
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            consts::DEFAULT_HOTKEY_AUTO_REFINE,
            consts::DEFAULT_HOTKEY_QUIT,
            consts::DEFAULT_HOTKEY_UNDO,
            consts::DEFAULT_HOTKEY_OCR,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_UNDO,
            consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD_PASTE,
        ] {
            parse_hotkey_binding(binding).unwrap_or_else(|e| panic!("{binding}: {e}"));
        }
    }

    /// 最終フォールバックが自動変換切替ホットキーと一致すること
    #[test]
    fn last_resort_hotkey_matches_auto_refine_default() {
        assert_eq!(
            parse_hotkey_or_default("", "also-invalid"),
            parse_hotkey_binding(consts::DEFAULT_HOTKEY_AUTO_REFINE).expect("auto_refine default")
        );
    }

    /// 修飾キー表記のゆらぎを許容すること
    #[test]
    fn test_parse_modifier_aliases() {
        assert!(parse_hotkey_binding("ctrl+alt+shift+f1").is_ok());
        assert!(parse_hotkey_binding("Control+Shift+A").is_ok());
    }

    /// 数字キーを解析できるようにすること
    #[test]
    fn test_parse_digit_keys() {
        assert!(parse_hotkey_binding("Alt+Shift+1").is_ok());
        assert!(parse_hotkey_binding("Alt+Shift+0").is_ok());
    }

    /// 追加したキー名と別名を解析できるようにすること
    #[test]
    fn test_parse_extended_keys() {
        for (binding, key) in [
            ("Ctrl+F13", HotkeyKey::F13),
            ("Ctrl+F24", HotkeyKey::F24),
            ("Alt+Space", HotkeyKey::Space),
            ("Ctrl+Shift+Enter", HotkeyKey::Enter),
            ("Ctrl+Shift+Return", HotkeyKey::Enter),
            ("Ctrl+Alt+Esc", HotkeyKey::Escape),
            ("Alt+Shift+Del", HotkeyKey::Delete),
            ("Alt+Shift+PgUp", HotkeyKey::PageUp),
            ("Alt+Shift+Up", HotkeyKey::ArrowUp),
            ("Alt+Shift+ArrowRight", HotkeyKey::ArrowRight),
            ("Ctrl+PrintScreen", HotkeyKey::PrintScreen),
            ("Ctrl+Numpad0", HotkeyKey::Numpad0),
            ("Ctrl+numpadadd", HotkeyKey::NumpadAdd),
            ("Alt+Shift+-", HotkeyKey::Minus),
            ("Alt+Shift+Minus", HotkeyKey::Minus),
            ("Alt+Shift+,", HotkeyKey::Comma),
            ("Alt+Shift+.", HotkeyKey::Period),
            ("Alt+Shift+/", HotkeyKey::Slash),
        ] {
            let parsed = parse_hotkey_binding(binding).unwrap_or_else(|e| panic!("{binding}: {e}"));
            assert_eq!(parsed.key, key, "{binding}");
        }
    }

    /// 不正なホットキーはエラーを返すようにすること
    #[test]
    fn test_parse_invalid_hotkey() {
        assert!(matches!(
            parse_hotkey_binding(""),
            Err(HotkeyParseError::Empty)
        ));
        assert!(matches!(
            parse_hotkey_binding("Alt+Unknown"),
            Err(HotkeyParseError::UnsupportedKey(_))
        ));
        assert!(matches!(
            parse_hotkey_binding("Alt+Shift+F99"),
            Err(HotkeyParseError::UnsupportedKey(_))
        ));
        for binding in ["Alt+Shift+F25", "Alt+;", "Alt+Numpad10", "Alt+CapsLock"] {
            assert!(
                matches!(
                    parse_hotkey_binding(binding),
                    Err(HotkeyParseError::UnsupportedKey(_))
                ),
                "{binding}"
            );
        }
    }
}

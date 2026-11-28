use clip_refiner_core::hotkey_binding::{HotkeyKey, HotkeyParseError, ParsedHotkey};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

// ======================================================================
// global-hotkey 変換
// ======================================================================
/// 解析済みホットキーを `global-hotkey` の `HotKey` へ変換する
pub(crate) fn to_global_hotkey(parsed: ParsedHotkey) -> HotKey {
    let mut modifiers = Modifiers::empty();
    if parsed.modifiers.alt {
        modifiers |= Modifiers::ALT;
    }
    if parsed.modifiers.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if parsed.modifiers.control {
        modifiers |= Modifiers::CONTROL;
    }
    if parsed.modifiers.meta {
        modifiers |= Modifiers::META;
    }

    HotKey::new(Some(modifiers), hotkey_key_to_code(parsed.key))
}

/// ホットキー文字列を解析し `HotKey` を返す
///
/// # Arguments
/// * `binding` - `Alt+Shift+S` 形式の文字列
///
/// # Returns
/// * `Ok(HotKey)` - 登録可能なホットキー
///
/// # Errors
/// 解析に失敗した場合
pub(crate) fn parse_global_hotkey(binding: &str) -> Result<HotKey, HotkeyParseError> {
    clip_refiner_core::hotkey_binding::parse_hotkey_binding(binding).map(to_global_hotkey)
}

macro_rules! define_hotkey_key_to_code {
    ($($variant:ident => $code:ident),* $(,)?) => {
        /// `HotkeyKey` を `global-hotkey` の `Code` へ変換する
        fn hotkey_key_to_code(key: HotkeyKey) -> Code {
            match key {
                $(HotkeyKey::$variant => Code::$code,)*
            }
        }
    };
}

define_hotkey_key_to_code! {
    KeyA => KeyA,
    KeyB => KeyB,
    KeyC => KeyC,
    KeyD => KeyD,
    KeyE => KeyE,
    KeyF => KeyF,
    KeyG => KeyG,
    KeyH => KeyH,
    KeyI => KeyI,
    KeyJ => KeyJ,
    KeyK => KeyK,
    KeyL => KeyL,
    KeyM => KeyM,
    KeyN => KeyN,
    KeyO => KeyO,
    KeyP => KeyP,
    KeyQ => KeyQ,
    KeyR => KeyR,
    KeyS => KeyS,
    KeyT => KeyT,
    KeyU => KeyU,
    KeyV => KeyV,
    KeyW => KeyW,
    KeyX => KeyX,
    KeyY => KeyY,
    KeyZ => KeyZ,
    Digit0 => Digit0,
    Digit1 => Digit1,
    Digit2 => Digit2,
    Digit3 => Digit3,
    Digit4 => Digit4,
    Digit5 => Digit5,
    Digit6 => Digit6,
    Digit7 => Digit7,
    Digit8 => Digit8,
    Digit9 => Digit9,
    F1 => F1,
    F2 => F2,
    F3 => F3,
    F4 => F4,
    F5 => F5,
    F6 => F6,
    F7 => F7,
    F8 => F8,
    F9 => F9,
    F10 => F10,
    F11 => F11,
    F12 => F12,
    F13 => F13,
    F14 => F14,
    F15 => F15,
    F16 => F16,
    F17 => F17,
    F18 => F18,
    F19 => F19,
    F20 => F20,
    F21 => F21,
    F22 => F22,
    F23 => F23,
    F24 => F24,
    Backspace => Backspace,
    Space => Space,
    Enter => Enter,
    Tab => Tab,
    Escape => Escape,
    Insert => Insert,
    Delete => Delete,
    Home => Home,
    End => End,
    PageUp => PageUp,
    PageDown => PageDown,
    ArrowUp => ArrowUp,
    ArrowDown => ArrowDown,
    ArrowLeft => ArrowLeft,
    ArrowRight => ArrowRight,
    PrintScreen => PrintScreen,
    Pause => Pause,
    ScrollLock => ScrollLock,
    Numpad0 => Numpad0,
    Numpad1 => Numpad1,
    Numpad2 => Numpad2,
    Numpad3 => Numpad3,
    Numpad4 => Numpad4,
    Numpad5 => Numpad5,
    Numpad6 => Numpad6,
    Numpad7 => Numpad7,
    Numpad8 => Numpad8,
    Numpad9 => Numpad9,
    NumpadAdd => NumpadAdd,
    NumpadSubtract => NumpadSubtract,
    NumpadMultiply => NumpadMultiply,
    NumpadDivide => NumpadDivide,
    NumpadDecimal => NumpadDecimal,
    Minus => Minus,
    Comma => Comma,
    Period => Period,
    Slash => Slash,
    Semicolon => Semicolon,
    Equal => Equal,
    Backquote => Backquote,
    BracketLeft => BracketLeft,
    Backslash => Backslash,
    BracketRight => BracketRight,
    Quote => Quote,
    IntlBackslash => IntlBackslash,
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use clip_refiner_core::hotkey_binding::{HotkeyModifiers, parse_hotkey_binding};

    use super::*;

    /// core の解析結果が `global-hotkey` に変換できるようにすること
    #[test]
    fn parse_global_hotkey_matches_core_parser() {
        let binding = "Alt+Shift+S";
        let parsed = parse_hotkey_binding(binding).expect("parse");
        let hotkey = to_global_hotkey(parsed);
        assert_eq!(hotkey, parse_global_hotkey(binding).expect("global"));
    }

    /// 修飾キーが `HotkeyModifiers` から `Modifiers` へ反映されるようにすること
    #[test]
    fn modifiers_are_mapped() {
        let parsed = ParsedHotkey {
            modifiers: HotkeyModifiers {
                alt: true,
                shift: true,
                control: false,
                meta: false,
            },
            key: HotkeyKey::KeyS,
        };
        let hotkey = to_global_hotkey(parsed);
        assert_eq!(hotkey.mods, Modifiers::ALT | Modifiers::SHIFT);
        assert_eq!(hotkey.key, Code::KeyS);
    }
}

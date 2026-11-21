use global_hotkey::hotkey::{Code, HotKey, Modifiers};

// ======================================================================
// ホットキー解析
// ======================================================================
/// `Alt+Shift+S` 形式の文字列を `HotKey` に変換する
///
/// 修飾キーは `Alt` / `Shift` / `Ctrl` / `Control` / `Meta` / `Super` / `Win` に対応する
/// キーは `A`〜`Z` または `F1`〜`F12` に対応する
///
/// # Arguments
/// * `binding` - ホットキー文字列
///
/// # Returns
/// * `Ok(HotKey)` - 解析に成功した `HotKey`
/// * `Err(String)` - 解析に失敗した場合の理由
pub fn parse_hotkey_binding(binding: &str) -> Result<HotKey, String> {
    let parts: Vec<&str> = binding
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("空のホットキー指定".to_string());
    }

    let key_name = parts
        .last()
        .ok_or_else(|| "キーが指定されていません".to_string())?;
    let code = parse_key_code(key_name).ok_or_else(|| format!("未対応のキー: {key_name}"))?;

    let mut modifiers = Modifiers::empty();
    for part in &parts[..parts.len() - 1] {
        modifiers |= parse_modifier(part)?;
    }

    Ok(HotKey::new(Some(modifiers), code))
}

/// 設定値を解析し、失敗時はデフォルトへフォールバックする
///
/// # Arguments
/// * `binding` - 設定ファイルのホットキー文字列
/// * `default_binding` - フォールバック先のデフォルト文字列
/// * `label` - ログ出力用の設定名
///
/// # Returns
/// * `HotKey` - 登録可能なホットキー
pub fn resolve_hotkey(binding: &str, default_binding: &str, label: &str) -> HotKey {
    match parse_hotkey_binding(binding) {
        Ok(hotkey) => hotkey,
        Err(e) => {
            crate::log_warn!(
                "ホットキー設定 '{label}' の解析に失敗: {e} (指定値: '{binding}')。デフォルト '{default_binding}' を使用"
            );
            parse_hotkey_binding(default_binding)
                .unwrap_or_else(|e| panic!("デフォルトホットキー '{default_binding}' が無効: {e}"))
        }
    }
}

/// 修飾キー名を修飾キーに変換する
///
/// # Arguments
/// * `name` - 修飾キー名
///
/// # Returns
/// * `Result<Modifiers, String>` - 修飾キー
fn parse_modifier(name: &str) -> Result<Modifiers, String> {
    match name.to_lowercase().as_str() {
        "alt" => Ok(Modifiers::ALT),
        "shift" => Ok(Modifiers::SHIFT),
        "ctrl" | "control" => Ok(Modifiers::CONTROL),
        "meta" | "super" | "win" => Ok(Modifiers::META),
        other => Err(format!("未対応の修飾キー: {other}")),
    }
}

/// キー名をキーコードに変換する
///
/// # Arguments
/// * `name` - キー名
///
/// # Returns
/// * `Option<Code>` - キーコード
fn parse_key_code(name: &str) -> Option<Code> {
    let upper = name.to_uppercase();
    if upper.len() == 1 {
        let ch = upper.chars().next()?;
        if ch.is_ascii_digit() {
            return digit_to_code(ch);
        }
        return letter_to_code(ch);
    }

    if let Some(num) = upper.strip_prefix('F') {
        return match num.parse::<u8>().ok()? {
            1 => Some(Code::F1),
            2 => Some(Code::F2),
            3 => Some(Code::F3),
            4 => Some(Code::F4),
            5 => Some(Code::F5),
            6 => Some(Code::F6),
            7 => Some(Code::F7),
            8 => Some(Code::F8),
            9 => Some(Code::F9),
            10 => Some(Code::F10),
            11 => Some(Code::F11),
            12 => Some(Code::F12),
            _ => None,
        };
    }

    None
}

/// 数字をキーコードに変換する
fn digit_to_code(ch: char) -> Option<Code> {
    Some(match ch {
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        _ => return None,
    })
}

/// 文字をキーコードに変換する
///
/// # Arguments
/// * `ch` - 文字
///
/// # Returns
/// * `Option<Code>` - キーコード
fn letter_to_code(ch: char) -> Option<Code> {
    Some(match ch {
        'A' => Code::KeyA,
        'B' => Code::KeyB,
        'C' => Code::KeyC,
        'D' => Code::KeyD,
        'E' => Code::KeyE,
        'F' => Code::KeyF,
        'G' => Code::KeyG,
        'H' => Code::KeyH,
        'I' => Code::KeyI,
        'J' => Code::KeyJ,
        'K' => Code::KeyK,
        'L' => Code::KeyL,
        'M' => Code::KeyM,
        'N' => Code::KeyN,
        'O' => Code::KeyO,
        'P' => Code::KeyP,
        'Q' => Code::KeyQ,
        'R' => Code::KeyR,
        'S' => Code::KeyS,
        'T' => Code::KeyT,
        'U' => Code::KeyU,
        'V' => Code::KeyV,
        'W' => Code::KeyW,
        'X' => Code::KeyX,
        'Y' => Code::KeyY,
        'Z' => Code::KeyZ,
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

    /// デフォルトホットキー文字列が解析できること
    #[test]
    fn test_parse_default_hotkeys() {
        for binding in [
            consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
            consts::DEFAULT_HOTKEY_NOTIFICATION,
            consts::DEFAULT_HOTKEY_PAUSE,
            consts::DEFAULT_HOTKEY_QUIT,
            consts::DEFAULT_HOTKEY_UNDO,
            consts::DEFAULT_HOTKEY_OCR,
        ] {
            parse_hotkey_binding(binding).unwrap_or_else(|e| panic!("{binding}: {e}"));
        }
    }

    /// 修飾キー表記のゆらぎを許容すること
    #[test]
    fn test_parse_modifier_aliases() {
        assert!(parse_hotkey_binding("ctrl+alt+shift+f1").is_ok());
        assert!(parse_hotkey_binding("Control+Shift+A").is_ok());
    }

    /// 数字キーを解析できること
    #[test]
    fn test_parse_digit_keys() {
        assert!(parse_hotkey_binding("Alt+Shift+1").is_ok());
        assert!(parse_hotkey_binding("Alt+Shift+0").is_ok());
    }

    /// 不正なホットキーはエラーを返すこと
    #[test]
    fn test_parse_invalid_hotkey() {
        assert!(parse_hotkey_binding("").is_err());
        assert!(parse_hotkey_binding("Alt+Unknown").is_err());
        assert!(parse_hotkey_binding("Alt+Shift+F99").is_err());
    }
}

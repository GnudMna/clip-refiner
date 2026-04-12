use std::borrow::Cow;

/// 文字列をバックスラッシュでエスケープする
///
/// JSONやプログラム文字列内で特殊な意味を持つ文字（改行、タブ、ダブルクォートなど）の前に
/// バックスラッシュを挿入します。
///
/// # Arguments
/// * `input` - エスケープ対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ済みの文字列。変更がない場合は元の文字列への参照を返します。
pub fn escape_string(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut changed = false;

    for c in input.chars() {
        match c {
            '"' | '\\' | '/' | '\x08' | '\x0c' | '\n' | '\r' | '\t' => {
                result.push('\\');
                result.push(match c {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    '\x08' => 'b',
                    '\x0c' => 'f',
                    '\n' => 'n',
                    '\r' => 'r',
                    '\t' => 't',
                    _ => unreachable!(),
                });
                changed = true;
            }
            _ => result.push(c),
        }
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(input)
    }
}

/// 文字列のエスケープを解除する
///
/// バックスラッシュでエスケープされた特殊文字（`\n`, `\t` など）を元の文字に戻します。
///
/// # Arguments
/// * `input` - エスケープ解除対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ解除済みの文字列。変更がない場合は元の文字列への参照を返します。
pub fn unescape_string(input: &str) -> Cow<'_, str> {
    if !input.contains('\\') {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut changed = false;

    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
        {
            match next {
                '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' => {
                    chars.next();
                    result.push(match next {
                        '"' => '"',
                        '\\' => '\\',
                        '/' => '/',
                        'b' => '\x08',
                        'f' => '\x0c',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => unreachable!(),
                    });
                    changed = true;
                    continue;
                }
                'u' => {
                    chars.next(); // consume 'u'
                    let hex: String = std::iter::from_fn(|| chars.next()).take(4).collect();
                    if hex.len() == 4 {
                        if let Ok(code) = u16::from_str_radix(&hex, 16) {
                            // サロゲートペア上位 (U+D800..U+DBFF) の処理
                            if (0xD800..=0xDBFF).contains(&code) {
                                // \uXXXX の次に \uYYYY があるか確認
                                let mut temp_chars = chars.clone();
                                if temp_chars.next() == Some('\\') && temp_chars.next() == Some('u')
                                {
                                    let low_hex: String =
                                        std::iter::from_fn(|| temp_chars.next()).take(4).collect();
                                    if low_hex.len() == 4 {
                                        if let Ok(low) = u16::from_str_radix(&low_hex, 16) {
                                            if (0xDC00..=0xDFFF).contains(&low) {
                                                // サロゲートペアをUnicodeスカラー値に変換
                                                let scalar = 0x10000u32
                                                    + ((code as u32 - 0xD800) << 10)
                                                    + (low as u32 - 0xDC00);
                                                if let Some(ch) = char::from_u32(scalar) {
                                                    // 消費済みの分だけ chars を進める
                                                    chars.next(); // '\'
                                                    chars.next(); // 'u'
                                                    for _ in 0..4 {
                                                        chars.next();
                                                    }
                                                    result.push(ch);
                                                    changed = true;
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                                // サロゲートペアでない場合はそのまま出力
                                result.push_str(&format!("\\u{}", hex));
                            } else if let Some(ch) = char::from_u32(code as u32) {
                                result.push(ch);
                                changed = true;
                                continue;
                            } else {
                                result.push_str(&format!("\\u{}", hex));
                            }
                        } else {
                            result.push_str(&format!("\\u{}", hex));
                        }
                    } else {
                        result.push_str(&format!("\\u{}", hex));
                    }
                    changed = true;
                    continue;
                }
                _ => {}
            }
        }
        result.push(c);
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(input)
    }
}

/// 正規表現のメタ文字をエスケープする
///
/// 正規表現内で特殊な意味を持つ文字（`.`, `*`, `+` など）の前にバックスラッシュを挿入し、
/// リテラルとして扱えるようにします。
///
/// # Arguments
/// * `input` - エスケープ対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ済みの文字列。変更がない場合は元の文字列への参照を返します。
pub fn regex_escape(input: &str) -> Cow<'_, str> {
    let result = regex::escape(input);
    if result == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(result)
    }
}

/// 正規表現のエスケープを解除する（簡易版）
///
/// 正規表現のメタ文字の前に付与されたバックスラッシュを削除します。
///
/// # Arguments
/// * `input` - エスケープ解除対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ解除済みの文字列。変更がない場合は元の文字列への参照を返します。
pub fn regex_unescape(input: &str) -> Cow<'_, str> {
    if !input.contains('\\') {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut changed = false;

    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
            && "\\^$.|?*+()[]{}".contains(next)
        {
            chars.next();
            result.push(next);
            changed = true;
            continue;
        }
        result.push(c);
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("plain"), "plain");
        assert!(matches!(escape_string("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn test_unescape_string() {
        assert_eq!(unescape_string("hello\\nworld"), "hello\nworld");
        assert_eq!(unescape_string("plain"), "plain");
        assert!(matches!(unescape_string("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn test_unescape_unicode() {
        // 基本的な \uXXXX
        assert_eq!(unescape_string("\\u0041"), "A");
        assert_eq!(unescape_string("\\u3042"), "あ");
        // テキスト中に埋め込まれた \uXXXX
        assert_eq!(unescape_string("hello\\u0020world"), "hello world");
        // サロゲートペア (U+1F600 GRINNING FACE)
        assert_eq!(unescape_string("\\uD83D\\uDE00"), "\u{1F600}");
        // 不完全な \uXXXX はそのまま残す
        assert_eq!(unescape_string("\\u004"), "\\u004");
        // 無効な16進数はそのまま残す
        assert_eq!(unescape_string("\\uGHIJ"), "\\uGHIJ");
    }

    #[test]
    fn test_regex_escape() {
        assert_eq!(regex_escape("h.w"), "h\\.w");
        assert_eq!(regex_escape("plain"), "plain");
        assert!(matches!(regex_escape("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn test_regex_unescape() {
        assert_eq!(regex_unescape("h\\.w"), "h.w");
        assert_eq!(regex_unescape("plain"), "plain");
        assert!(matches!(regex_unescape("plain"), Cow::Borrowed(_)));
    }
}

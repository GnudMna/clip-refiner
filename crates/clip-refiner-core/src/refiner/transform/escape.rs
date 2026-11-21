use std::borrow::Cow;
use std::fmt::Write;

// ======================================================================
// エスケープ
// ======================================================================
/// 文字列をバックスラッシュでエスケープする
///
/// JSONやプログラム文字列内で特殊な意味を持つ文字(改行、タブ、ダブルクォートなど)の前に
/// バックスラッシュを挿入する
///
/// # Arguments
/// * `input` - エスケープ対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ済みの文字列。変更がない場合は元の文字列への参照を返す。
pub fn escape_string(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut changed = false;

    for c in input.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '/' => result.push_str("\\/"),
            '\x08' => result.push_str("\\b"),
            '\x0c' => result.push_str("\\f"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // 上記以外の制御文字は JSON 文字列内で \uXXXX 形式にする必要がある
            c if c < '\x20' => {
                let _ = write!(result, "\\u{:04x}", u32::from(c));
            }
            _ => {
                result.push(c);
                continue;
            }
        }
        changed = true;
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(input)
    }
}

// ======================================================================
// アンエスケープ
// ======================================================================
/// 文字列のエスケープを解除する
///
/// バックスラッシュでエスケープされた特殊文字(`\n`, `\t` など)を元の文字に戻す
/// 4 桁の 16 進数が続かない `\u` や、対にならないサロゲートはそのまま残す
///
/// # Arguments
/// * `input` - エスケープ解除対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ解除済みの文字列。変更がない場合は元の文字列への参照を返す。
pub fn unescape_string(input: &str) -> Cow<'_, str> {
    if !input.contains('\\') {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut rest = input;
    let mut changed = false;

    while let Some(pos) = rest.find('\\') {
        result.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        if let Some((ch, consumed)) = decode_escape(after) {
            result.push(ch);
            rest = &after[consumed..];
            changed = true;
        } else {
            result.push('\\');
            rest = after;
        }
    }
    result.push_str(rest);

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(input)
    }
}

/// バックスラッシュの直後にあるエスケープシーケンスを 1 つ解釈する
///
/// # Arguments
/// * `after` - バックスラッシュの次の文字から始まる文字列
///
/// # Returns
/// * `Option<(char, usize)>` - 解釈できた場合は `Some((変換後の文字, 消費したバイト数))`、
///   解釈できない場合は `None`
fn decode_escape(after: &str) -> Option<(char, usize)> {
    let simple = match after.as_bytes().first()? {
        b'"' => Some('"'),
        b'\\' => Some('\\'),
        b'/' => Some('/'),
        b'b' => Some('\x08'),
        b'f' => Some('\x0c'),
        b'n' => Some('\n'),
        b'r' => Some('\r'),
        b't' => Some('\t'),
        _ => None,
    };
    if let Some(ch) = simple {
        return Some((ch, 1));
    }
    if !after.starts_with('u') {
        return None;
    }

    let code = u32::from(parse_hex4(after.get(1..5)?)?);
    if (0xD800..=0xDBFF).contains(&code) {
        // サロゲートペア上位は、直後の \uXXXX が下位 (U+DC00..U+DFFF) のときだけ結合する
        let low_hex = after.get(5..)?.strip_prefix("\\u")?.get(..4)?;
        let low = u32::from(parse_hex4(low_hex)?);
        if !(0xDC00..=0xDFFF).contains(&low) {
            return None;
        }
        let scalar = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
        return Some((char::from_u32(scalar)?, 11));
    }
    Some((char::from_u32(code)?, 5))
}

/// 4 桁の 16 進数文字列を `u16` に変換する
///
/// `u16::from_str_radix` は先頭の `+` を受け付けるため、全桁が 16 進数字かを先に確認する
///
/// # Arguments
/// * `hex` - 変換対象の文字列
///
/// # Returns
/// * `Option<u16>` - 4 文字すべてが 16 進数字の場合は `Some(値)`、それ以外は `None`
fn parse_hex4(hex: &str) -> Option<u16> {
    if hex.len() == 4 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        None
    }
}

// ======================================================================
// 正規表現エスケープ
// ======================================================================
/// `regex::escape` がエスケープする文字
///
/// `regex_unescape` の解除対象と一致させ、`regex_escape` の逆変換にする
const REGEX_META_CHARS: &str = "\\.+*?()|[]{}^$#&-~";

/// 正規表現のメタ文字をエスケープする
///
/// 正規表現内で特殊な意味を持つ文字(`.`, `*`, `+` など)の前にバックスラッシュを挿入し、
/// リテラルとして扱えるようにする
///
/// # Arguments
/// * `input` - エスケープ対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ済みの文字列。変更がない場合は元の文字列への参照を返す。
pub fn regex_escape(input: &str) -> Cow<'_, str> {
    let result = regex::escape(input);
    if result == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(result)
    }
}

/// 正規表現のエスケープを解除する(簡易版)
///
/// 正規表現のメタ文字の前に付与されたバックスラッシュを削除する
///
/// # Arguments
/// * `input` - エスケープ解除対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - エスケープ解除済みの文字列。変更がない場合は元の文字列への参照を返す。
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
            && REGEX_META_CHARS.contains(next)
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 改行を含む文字列のエスケープと Borrowed 返却できるようにすること
    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("plain"), "plain");
        assert!(matches!(escape_string("plain"), Cow::Borrowed(_)));
    }

    /// アンエスケープの基本動作できるようにすること
    #[test]
    fn test_unescape_string() {
        assert_eq!(unescape_string("hello\\nworld"), "hello\nworld");
        assert_eq!(unescape_string("plain"), "plain");
        assert!(matches!(unescape_string("plain"), Cow::Borrowed(_)));
    }

    /// \\uXXXX およびサロゲートペアのアンエスケープできるようにすること
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
        // 先頭の符号は16進数字ではないためそのまま残す
        assert_eq!(unescape_string("\\u+041"), "\\u+041");
        assert_eq!(unescape_string("\\u-041"), "\\u-041");
        // サロゲートペア下位側の符号もそのまま残す
        assert_eq!(unescape_string("\\uD83D\\u+E00"), "\\uD83D\\u+E00");
    }

    /// `regex_escape` の基本動作できるようにすること
    #[test]
    fn test_regex_escape() {
        assert_eq!(regex_escape("h.w"), "h\\.w");
        assert_eq!(regex_escape("plain"), "plain");
        assert!(matches!(regex_escape("plain"), Cow::Borrowed(_)));
    }

    /// `regex_unescape` の基本動作できるようにすること
    #[test]
    fn test_regex_unescape() {
        assert_eq!(regex_unescape("h\\.w"), "h.w");
        assert_eq!(regex_unescape("plain"), "plain");
        assert!(matches!(regex_unescape("plain"), Cow::Borrowed(_)));
    }

    /// `escape_string`: すべての対象文字が正しくエスケープされるようにすること
    #[test]
    fn test_escape_string_all_chars() {
        assert_eq!(escape_string("\""), "\\\"");
        assert_eq!(escape_string("\\"), "\\\\");
        assert_eq!(escape_string("/"), "\\/");
        assert_eq!(escape_string("\x08"), "\\b");
        assert_eq!(escape_string("\x0c"), "\\f");
        assert_eq!(escape_string("\n"), "\\n");
        assert_eq!(escape_string("\r"), "\\r");
        assert_eq!(escape_string("\t"), "\\t");
    }

    /// `escape_string`: 名前付きエスケープを持たない制御文字を `\u00XX` にすること
    #[test]
    fn test_escape_string_control_chars() {
        assert_eq!(escape_string("\0"), "\\u0000");
        assert_eq!(escape_string("a\x01b"), "a\\u0001b");
        assert_eq!(escape_string("\x1b[0m"), "\\u001b[0m");
        assert_eq!(escape_string("\x1f"), "\\u001f");
        // U+0020 以上は対象外
        assert!(matches!(escape_string(" \x7f"), Cow::Borrowed(_)));
        // 全制御文字がエスケープ後に制御文字を含まず、アンエスケープで元に戻る
        let all: String = ('\0'..'\x20').collect();
        let escaped = escape_string(&all);
        assert!(escaped.chars().all(|c| c >= '\x20'));
        assert_eq!(unescape_string(&escaped), all);
    }

    /// `unescape_string`: 4 桁に満たない `\u` は後続のエスケープを巻き込まず、変更なしとして扱うこと
    #[test]
    fn test_unescape_string_incomplete_unicode() {
        assert_eq!(unescape_string("\\u12\\n"), "\\u12\n");
        assert_eq!(unescape_string("\\u12 \\t"), "\\u12 \t");
        // 変換が 1 つもない場合は Borrowed を返す
        assert!(matches!(unescape_string("\\u12"), Cow::Borrowed(_)));
        assert!(matches!(unescape_string("\\uGHIJ"), Cow::Borrowed(_)));
        assert!(matches!(unescape_string("\\uD83D"), Cow::Borrowed(_)));
        assert!(matches!(unescape_string("\\uDE00"), Cow::Borrowed(_)));
        assert!(matches!(unescape_string("end\\"), Cow::Borrowed(_)));
        // 上位サロゲートの後ろが下位でない場合は、後続のエスケープだけ変換する
        assert_eq!(unescape_string("\\uD83D\\u0041"), "\\uD83DA");
        // 4 バイト目がマルチバイト文字の途中になっても panic しない
        assert_eq!(unescape_string("\\uあいう"), "\\uあいう");
    }

    /// `escape_string`: 空文字列は Borrowed を返すようにすること
    #[test]
    fn test_escape_string_empty() {
        assert!(matches!(escape_string(""), Cow::Borrowed(_)));
    }

    /// `unescape_string`: すべての対象シーケンスが正しくアンエスケープされるようにすること
    #[test]
    fn test_unescape_string_all_sequences() {
        assert_eq!(unescape_string("\\\""), "\"");
        assert_eq!(unescape_string("\\\\"), "\\");
        assert_eq!(unescape_string("\\/"), "/");
        assert_eq!(unescape_string("\\b"), "\x08");
        assert_eq!(unescape_string("\\f"), "\x0c");
        assert_eq!(unescape_string("\\n"), "\n");
        assert_eq!(unescape_string("\\r"), "\r");
        assert_eq!(unescape_string("\\t"), "\t");
    }

    /// `unescape_string`: バックスラッシュを含まない文字列は Borrowed を返すようにすること
    #[test]
    fn test_unescape_string_no_backslash() {
        assert!(matches!(unescape_string("hello world"), Cow::Borrowed(_)));
    }

    /// `regex_unescape`: メタ文字以外のバックスラッシュはそのまま残るようにすること
    #[test]
    fn test_regex_unescape_non_meta() {
        // 'a' は正規表現メタ文字でないのでそのまま
        assert_eq!(regex_unescape("\\a"), "\\a");
    }

    /// `regex_escape` / `regex_unescape` の往復変換できるようにすること
    #[test]
    fn test_regex_escape_unescape_roundtrip() {
        let original = "a.b*c+d?e";
        let escaped = regex_escape(original);
        let unescaped = regex_unescape(&escaped);
        assert_eq!(unescaped, original);
    }

    /// `regex::escape` がエスケープする全メタ文字を `regex_unescape` で戻せるようにすること
    ///
    /// `-` `#` `&` `~` を含む
    #[test]
    fn test_regex_unescape_all_meta_chars() {
        for c in REGEX_META_CHARS.chars() {
            let original = format!("a{c}b");
            let escaped = regex_escape(&original);
            assert_eq!(
                escaped,
                format!("a\\{c}b"),
                "regex::escape が {c:?} をエスケープしない"
            );
            assert_eq!(regex_unescape(&escaped), original);
        }
        assert_eq!(regex_unescape("a\\-b"), "a-b");
        assert_eq!(regex_unescape("\\#\\&\\~"), "#&~");
    }
}

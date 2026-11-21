use std::borrow::Cow;

// ======================================================================
// 定数
// ======================================================================
/// 空白文字に加えてトリム対象にする、コピー元によって混ざる見えない文字
///
/// BOM (`U+FEFF`) とゼロ幅スペース (`U+200B`)。どちらも `char::is_whitespace` は `false` を返す
const INVISIBLE_TRIM_CHARS: [char; 2] = ['\u{FEFF}', '\u{200B}'];

// ======================================================================
// 全体トリム
// ======================================================================
/// 文字列全体の前後にある空白文字を削除する
///
/// 入力文字列の最初と最後にあるスペース、タブ、改行などの空白文字を取り除く
/// BOM (`U+FEFF`) とゼロ幅スペース (`U+200B`) も取り除く
///
/// # Arguments
/// * `input` - トリム対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 前後の空白が削除された文字列
pub fn trim_text(input: &str) -> Cow<'_, str> {
    Cow::Borrowed(trim_edges(input))
}

// ======================================================================
// 行ごとトリム
// ======================================================================
/// 文字列の各行について、前後の空白文字を削除する
///
/// 行ごとにトリムを行い、各行の改行コードは元のまま保持する (CRLF と LF が混在していても変えない)
/// 削除する文字は `trim_text` と同じ (全角スペース・BOM・ゼロ幅スペースを含む)
///
/// # Arguments
/// * `input` - 各行をトリムする対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 各行の前後の空白が削除された文字列。変更がない場合は元の文字列への参照を返す
pub fn trim_lines(input: &str) -> Cow<'_, str> {
    super::utils::process_lines(input, |line| Some(Cow::Borrowed(trim_edges(line))))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 前後の空白文字・BOM・ゼロ幅スペースを取り除く
fn trim_edges(input: &str) -> &str {
    input.trim_matches(|c: char| c.is_whitespace() || INVISIBLE_TRIM_CHARS.contains(&c))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 文字列全体のトリムできるようにすること
    #[test]
    fn test_trim_text() {
        assert_eq!(trim_text("  hello  "), "hello");
        assert_eq!(trim_text("\n world \r\n"), "world");
    }

    /// 各行ごとのLF 区切りで各行をトリムできるようにすること
    #[test]
    fn test_trim_lines_lf() {
        let input = "  hello  \n  world \n  rust ";
        let expected = "hello\nworld\nrust";
        assert_eq!(trim_lines(input), expected);
    }

    /// 各行ごとのCRLF を保持して各行をトリムできるようにすること
    #[test]
    fn test_trim_lines_crlf() {
        let input = "  hello  \r\n  world \r\n  rust ";
        let expected = "hello\r\nworld\r\nrust";
        assert_eq!(trim_lines(input), expected);
    }

    /// CRLF と LF が混在していても、各行の改行コードを保持して各行をトリムすること
    #[test]
    fn test_trim_lines_mixed_line_endings() {
        assert_eq!(trim_lines(" a \r\n b \n c \r\n"), "a\r\nb\nc\r\n");
        assert_eq!(trim_lines(" a \n b \r\n c "), "a\nb\r\nc");
        // 変更がなければ元の文字列を借用する
        assert!(matches!(trim_lines("a\r\nb\n"), Cow::Borrowed(_)));
    }

    /// 全角スペースも `trim_text` と同様に削除すること
    #[test]
    fn test_trim_lines_ideographic_space() {
        assert_eq!(
            trim_lines("\u{3000}hello\u{3000}\n \u{3000}world"),
            "hello\nworld"
        );
        assert_eq!(trim_lines("\u{3000}a\u{3000}\r\n\u{3000}b"), "a\r\nb");
        assert_eq!(
            trim_lines("\u{3000}x\u{3000}"),
            trim_text("\u{3000}x\u{3000}")
        );
    }

    /// BOM とゼロ幅スペースも前後から削除し、途中にあるものは残すこと
    #[test]
    fn test_trim_text_invisible_chars() {
        assert_eq!(trim_text("\u{FEFF}hello"), "hello");
        assert_eq!(trim_text("\u{200B} \u{200B}hello\u{200B}\n"), "hello");
        assert_eq!(trim_text("a\u{200B}b"), "a\u{200B}b");
        assert_eq!(trim_text("\u{FEFF}\u{200B}"), "");
    }

    /// 行ごとのトリムでも BOM とゼロ幅スペースを各行の前後から削除すること
    #[test]
    fn test_trim_lines_invisible_chars() {
        assert_eq!(
            trim_lines("\u{FEFF}a\u{200B}\r\n\u{200B} b \u{FEFF}\nc\u{200B}d"),
            "a\r\nb\nc\u{200B}d"
        );
    }

    /// 空文字列に対する行トリムできるようにすること
    #[test]
    fn test_trim_lines_empty() {
        let input = "";
        let expected = "";
        assert_eq!(trim_lines(input), expected);
    }

    /// 空白文字のみの行をトリムして空行にすること
    #[test]
    fn test_trim_lines_whitespace_only() {
        let input = "  \n\t\n";
        let expected = "\n\n";
        assert_eq!(trim_lines(input), expected);
    }
}

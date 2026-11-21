use std::borrow::Cow;

// ======================================================================
// 全体トリム
// ======================================================================
/// 文字列全体の前後にある空白文字を削除する
///
/// 入力文字列の最初と最後にあるスペース、タブ、改行などの空白文字を取り除く
///
/// # Arguments
/// * `input` - トリム対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 前後の空白が削除された文字列
pub fn trim_text(input: &str) -> Cow<'_, str> {
    Cow::Borrowed(input.trim())
}

// ======================================================================
// 行ごとトリム
// ======================================================================
/// 文字列の各行について、前後の空白文字を削除する
///
/// 行ごとにトリムを行い、各行の改行コードは元のまま保持する (CRLF と LF が混在していても変えない)
/// 削除する空白文字は `trim_text` と同じ (全角スペースを含む)
///
/// # Arguments
/// * `input` - 各行をトリムする対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 各行の前後の空白が削除された文字列。変更がない場合は元の文字列への参照を返す。
pub fn trim_lines(input: &str) -> Cow<'_, str> {
    super::utils::process_lines(input, |line| Some(Cow::Borrowed(line.trim())))
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

    /// 空文字列に対する行トリムできるようにすること
    #[test]
    fn test_trim_lines_empty() {
        let input = "";
        let expected = "";
        assert_eq!(trim_lines(input), expected);
    }

    /// 空白文字のみの行に対するトリムできるようにすること
    /// 空行になることを確認できるようにすること
    #[test]
    fn test_trim_lines_whitespace_only() {
        let input = "  \n\t\n";
        let expected = "\n\n";
        assert_eq!(trim_lines(input), expected);
    }
}

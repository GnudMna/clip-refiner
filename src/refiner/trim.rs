/// 文字列の前後空白を削除する
///
/// # Arguments
/// * `input` - トリムする文字列。
///
/// # Returns
/// * `String` - 前後の空白が削除された文字列。
pub fn trim_text(input: &str) -> String {
    input.trim().to_string()
}

/// 文字列の各行の前後空白を削除する
///
/// # Arguments
/// * `input` - 各行をトリムする文字列。
///
/// # Returns
/// * `String` - 各行の前後の空白が削除された文字列。
pub fn trim_lines(input: &str) -> String {
    input
        .split('\n')
        .map(|line| line.trim_matches(['\r', ' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 文字列全体のトリムテスト
    #[test]
    fn test_trim_text() {
        assert_eq!(trim_text("  hello  "), "hello");
        assert_eq!(trim_text("\n world \r\n"), "world");
    }

    /// 各行ごとのトリムテスト
    #[test]
    fn test_trim_lines() {
        let input = "  hello  \n  world \r\n  rust ";
        let expected = "hello\nworld\nrust";
        let actual = trim_lines(input);
        assert_eq!(actual, expected);
    }

    /// 空文字列に対する行トリムテスト
    #[test]
    fn test_trim_lines_empty() {
        let input = "";
        let expected = "";
        assert_eq!(trim_lines(input), expected);
    }

    /// 空白文字のみの行に対するトリムテスト
    /// 空行になることを確認
    #[test]
    fn test_trim_lines_whitespace_only() {
        let input = "  \n\t\n";
        let expected = "\n\n";
        assert_eq!(trim_lines(input), expected);
    }
}

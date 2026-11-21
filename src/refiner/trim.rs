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
        .split_inclusive(|c| c == '\n' || c == '\r')
        .map(|chunk| {
            // 改行コード部分と本文部分を分離
            let trimmed = chunk.trim_matches(['\n', '\r']);
            let newline = &chunk[trimmed.len()..];
            format!("{}{}", trimmed.trim(), newline)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_text() {
        assert_eq!(trim_text("  hello  "), "hello");
        assert_eq!(trim_text("\n world \r\n"), "world");
    }

    #[test]
    fn test_trim_lines() {
        let input = "  hello  \n  world \r\n  rust ";
        let expected = "hello\nworld\r\nrust";
        assert_eq!(trim_lines(input), expected);
    }
}

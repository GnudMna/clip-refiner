use std::borrow::Cow;

// ======================================================================
// 全体トリム
// ======================================================================
/// 文字列全体の前後にある空白文字を削除する
///
/// 入力文字列の最初と最後にあるスペース、タブ、改行などの空白文字を取り除きます。
///
/// # Arguments
/// * `input` - トリム対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 前後の空白が削除された文字列。
pub fn trim_text(input: &str) -> Cow<'_, str> {
    Cow::Borrowed(input.trim())
}

// ======================================================================
// 行ごとトリム
// ======================================================================
/// 文字列の各行について、前後の空白文字を削除する
///
/// 行ごとにトリムを行い、改行で結合し直します。
/// 各行末の `\r` も適切に処理されます。
///
/// # Arguments
/// * `input` - 各行をトリムする対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 各行の前後の空白が削除された文字列。変更がない場合は元の文字列への参照を返します。
pub fn trim_lines(input: &str) -> Cow<'_, str> {
    let result = input
        .split('\n')
        .map(|line| line.trim_matches(['\r', ' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n");

    if result == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(result)
    }
}

// ======================================================================
// テスト
// ======================================================================
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

use std::borrow::Cow;

// ======================================================================
// カンマ付与
// ======================================================================
/// 数値に3桁区切りのカンマを付与する
///
/// 複数行のテキストは行ごとに処理し、数値として認識できない行は元のまま残す
///
/// # Arguments
/// * `text` - カンマを付与する対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 3桁ごとにカンマが付与された文字列。数値として認識できない場合は元の文字列を返す
pub fn add_commas(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| Some(add_commas_single(line)))
}

/// 1 行分の数値に3桁区切りのカンマを付与する
///
/// 入力された文字列が数値として妥当な場合、整数部分に3桁ごとのカンマを挿入する
/// 負の数や小数を含む数値にも対応している
/// 整数部が先頭ゼロで始まる値 (`00123456` など) は数値として扱わず元のまま返す
///
/// # Arguments
/// * `text` - カンマを付与する対象の文字列 (改行を含まない)
///
/// # Returns
/// * `Cow<'_, str>` - 3桁ごとにカンマが付与された文字列。数値として認識できない場合は元の文字列を返す
fn add_commas_single(text: &str) -> Cow<'_, str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed(text);
    }

    // 数値、カンマ、小数点以外の文字が含まれているかチェック
    // カンマが含まれていても、最終的に一貫した形式にするために一旦除去して再付与するアプローチを取る
    if !is_numeric_input(trimmed) {
        return Cow::Borrowed(text);
    }

    // カンマを除去して純粋な数値にする
    let pure_numeric = trimmed.replace(',', "");

    // 整数部と小数部に分ける
    let parts: Vec<&str> = pure_numeric.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

    let is_negative = integer_part.starts_with('-');
    let abs_integer = if is_negative {
        &integer_part[1..]
    } else {
        integer_part
    };

    // 先頭ゼロのある整数部は ID・電話番号・郵便番号などの可能性があるため変換しない
    if abs_integer.len() > 1 && abs_integer.starts_with('0') {
        return Cow::Borrowed(text);
    }

    // 整数部にカンマを付与
    let mut result = String::new();

    for (count, c) in abs_integer.chars().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }

    let mut formatted_int: String = result.chars().rev().collect();
    if is_negative {
        formatted_int.insert(0, '-');
    }

    let final_result = if !decimal_part.is_empty() {
        format!("{formatted_int}.{decimal_part}")
    } else if pure_numeric.contains('.') {
        // 元々小数点が末尾にあった場合
        format!("{formatted_int}.")
    } else {
        formatted_int
    };

    restore_surrounding_whitespace(text, trimmed, &final_result)
}

/// 変換結果に元の前後の空白を戻す
///
/// 変換結果が trim 済みの元の値と同じ場合は元の文字列をそのまま返す
///
/// # Arguments
/// * `text` - 元の文字列 (前後の空白を含む)
/// * `trimmed` - `text` を trim した文字列
/// * `converted` - `trimmed` を変換した文字列
///
/// # Returns
/// * `Cow<'_, str>` - 前後の空白を保った変換結果。変更がなければ `text`
fn restore_surrounding_whitespace<'a>(
    text: &'a str,
    trimmed: &str,
    converted: &str,
) -> Cow<'a, str> {
    if converted == trimmed {
        return Cow::Borrowed(text);
    }
    let indent_len = text.len() - text.trim_start().len();
    let trailing_start = indent_len + trimmed.len();
    Cow::Owned(format!(
        "{}{converted}{}",
        &text[..indent_len],
        &text[trailing_start..]
    ))
}

// ======================================================================
// カンマ除去
// ======================================================================
/// 数値からカンマを除去する
///
/// 複数行のテキストは行ごとに処理し、数値として認識できない行は元のまま残す
///
/// # Arguments
/// * `text` - カンマを除去する対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - カンマが除去された文字列。数値として認識できない場合は元の文字列を返す
pub fn remove_commas(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| Some(remove_commas_single(line)))
}

/// 1 行分の数値からカンマを除去する
///
/// 入力された文字列に含まれるすべてのカンマを削除する
/// 数値として妥当な形式である場合にのみ処理を行う
///
/// # Arguments
/// * `text` - カンマを除去する対象の文字列 (改行を含まない)
///
/// # Returns
/// * `Cow<'_, str>` - カンマが除去された文字列。数値として認識できない場合は元の文字列を返す
fn remove_commas_single(text: &str) -> Cow<'_, str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed(text);
    }

    if !is_numeric_input(trimmed) {
        return Cow::Borrowed(text);
    }

    restore_surrounding_whitespace(text, trimmed, &trimmed.replace(',', ""))
}

// ======================================================================
// 数値判定
// ======================================================================
#[derive(PartialEq, Clone, Copy)]
enum NumericPrev {
    Start,
    Digit,
    Comma,
    Dot,
}

/// 入力が数値として妥当な形式か判定する
///
/// 数字、カンマ、小数点、および先頭のマイナス記号のみで構成されているかチェックする
/// 以下の場合は不当とみなす
/// - 先頭カンマ (`,123`)
/// - 末尾カンマ (`123,`)
/// - 連続カンマ (`1,,234`)
/// - 小数部のカンマ (`1.2,3`)
/// - 複数の小数点 (`1.2.3`)
/// - カンマ直後の小数点 (`1,.2`)
/// - 3 桁区切りでないカンマ位置 (`1,2,3` / `12,34` / `1234,567` / `1,23`)
///
/// # Arguments
/// * `text` - 判定対象の文字列
///
/// # Returns
/// * `bool` - 数値入力として妥当な場合は `true`、そうでない場合は `false`
fn is_numeric_input(text: &str) -> bool {
    let mut chars = text.chars().peekable();

    // マイナス記号のチェック
    if let Some('-') = chars.peek() {
        chars.next();
    }

    if chars.peek().is_none() {
        return false;
    }

    // 直前の文字種を追跡する
    let mut prev = NumericPrev::Start;
    let mut has_dot = false;
    // 直前のカンマ (または先頭) から続く整数部の桁数
    let mut group_len = 0usize;
    let mut has_comma = false;

    for c in chars {
        match c {
            '0'..='9' => {
                group_len += 1;
                prev = NumericPrev::Digit;
            }
            ',' => {
                // 先頭カンマ・連続カンマ・小数部のカンマはすべて不正
                if prev != NumericPrev::Digit || has_dot {
                    return false;
                }
                // 最初のグループは 1〜3 桁、以降のグループは直前のカンマ区切りが 3 桁ちょうど
                let valid_group = if has_comma {
                    group_len == 3
                } else {
                    group_len <= 3
                };
                if !valid_group {
                    return false;
                }
                has_comma = true;
                group_len = 0;
                prev = NumericPrev::Comma;
            }
            '.' => {
                // 複数の小数点・カンマ直後の小数点は不正
                if has_dot || prev == NumericPrev::Comma {
                    return false;
                }
                // 最後のグループも 3 桁ちょうどでなければならない
                if has_comma && group_len != 3 {
                    return false;
                }
                has_dot = true;
                prev = NumericPrev::Dot;
            }
            _ => return false,
        }
    }

    // 末尾カンマは不正。小数点なしで終わる場合も最後のグループは 3 桁ちょうど
    prev != NumericPrev::Comma && (has_dot || !has_comma || group_len == 3)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 3桁区切りのカンマを追加すること
    #[test]
    fn test_add_commas() {
        assert_eq!(add_commas("1234"), "1,234");
        assert_eq!(add_commas("1234567"), "1,234,567");
        assert_eq!(add_commas("123.456"), "123.456");
        assert_eq!(add_commas("1234.567"), "1,234.567");
        assert_eq!(add_commas("-1234567"), "-1,234,567");
        assert_eq!(add_commas("1,234,567"), "1,234,567");
        assert_eq!(add_commas("1234 yen"), "1234 yen");
        assert_eq!(add_commas(""), "");
    }

    /// カンマを除去すること
    #[test]
    fn test_remove_commas() {
        assert_eq!(remove_commas("1,234"), "1234");
        assert_eq!(remove_commas("1,234,567"), "1234567");
        assert_eq!(remove_commas("1,234.56"), "1234.56");
        assert_eq!(remove_commas("-1,234"), "-1234");
        assert_eq!(remove_commas("1,234 yen"), "1,234 yen");
        assert_eq!(remove_commas(""), "");
    }

    /// ゼロに対するカンマ追加できるようにすること
    #[test]
    fn test_add_commas_zero() {
        assert_eq!(add_commas("0"), "0");
        assert_eq!(add_commas("0.0"), "0.0");
    }

    /// 先頭ゼロのある整数部は ID や電話番号を壊さないよう変換しないようにすること
    #[test]
    fn test_add_commas_leading_zero_is_untouched() {
        for input in ["00123456", "0123", "-0123456", "0123456.78", "00,123,456"] {
            assert!(matches!(add_commas(input), Cow::Borrowed(_)), "{input}");
        }
        // 複数行では先頭ゼロの行だけが残る
        assert_eq!(add_commas("00123456\n1234"), "00123456\n1,234");
        // 整数部が 0 のみの値は従来どおり扱う
        assert_eq!(add_commas("0.5"), "0.5");
    }

    /// 負の数に対するカンマ追加できるようにすること
    #[test]
    fn test_add_commas_negative() {
        assert_eq!(add_commas("-1234.56"), "-1,234.56");
        assert_eq!(add_commas("-.5"), "-.5");
    }

    /// 数字・カンマ・ピリオド・マイナス記号以外を含む入力や複数の小数点を数値とみなさないようにすること
    #[test]
    fn test_is_numeric_input_check() {
        assert!(!is_numeric_input("abc"));
        assert!(!is_numeric_input("12.34.56"));
        assert!(is_numeric_input("-1,234.5"));
    }

    /// カンマ位置の不正パターンを弾くようにすること
    #[test]
    fn test_is_numeric_input_invalid_comma() {
        assert!(!is_numeric_input(",123"), "先頭カンマは不正");
        assert!(!is_numeric_input("123,"), "末尾カンマは不正");
        assert!(!is_numeric_input("1,,234"), "連続カンマは不正");
        assert!(!is_numeric_input("1.2,3"), "小数部のカンマは不正");
        assert!(!is_numeric_input("1,.2"), "カンマ直後の小数点は不正");
    }

    /// 3 桁区切りでないカンマ位置を数値として扱わないようにすること
    #[test]
    fn test_is_numeric_input_invalid_grouping() {
        assert!(!is_numeric_input("1,2,3"));
        assert!(!is_numeric_input("12,34"));
        assert!(!is_numeric_input("1234,567"), "先頭グループが 4 桁以上");
        assert!(!is_numeric_input("1,23"), "末尾グループが 3 桁未満");
        assert!(!is_numeric_input("1,2345"), "末尾グループが 3 桁超");
        assert!(!is_numeric_input("1,234,56"), "途中までは正しい");
        assert!(
            !is_numeric_input("1,23.5"),
            "小数点直前のグループが 3 桁未満"
        );
        assert!(!is_numeric_input("-1,2,3"));
    }

    /// カンマ位置が不正な文字列 (CSV 風の値) を変換せず元のまま返すようにすること
    #[test]
    fn test_comma_separated_values_are_untouched() {
        for input in ["1,2,3", "12,34", "1234,567", "1,23.5", "-1,2,3"] {
            assert!(matches!(add_commas(input), Cow::Borrowed(_)), "{input}");
            assert!(matches!(remove_commas(input), Cow::Borrowed(_)), "{input}");
        }
        // 複数行では不正な行だけが残る
        assert_eq!(add_commas("1,2,3\n1234"), "1,2,3\n1,234");
        assert_eq!(remove_commas("1,2,3\n1,234"), "1,2,3\n1234");
    }

    /// 正当なパターンを通過させるようにすること
    #[test]
    fn test_is_numeric_input_valid() {
        assert!(is_numeric_input("123,456"));
        assert!(is_numeric_input("12,345,678.9"));
        assert!(is_numeric_input("1,234."));
        assert!(is_numeric_input("-1,234"));
        assert!(is_numeric_input("1234"));
        assert!(is_numeric_input("1,234"));
        assert!(is_numeric_input("1,234,567"));
        assert!(is_numeric_input("-1234"));
        assert!(is_numeric_input(".5"));
        assert!(is_numeric_input("-.5"));
        assert!(is_numeric_input("1234."));
        assert!(is_numeric_input("0"));
    }

    /// 小数点のみ (整数部なし) の処理できるようにすること
    #[test]
    fn test_add_commas_decimal_only() {
        assert_eq!(add_commas(".5"), ".5");
        assert_eq!(add_commas("-.5"), "-.5");
    }

    /// 末尾に小数点がある値の処理できるようにすること
    #[test]
    fn test_add_commas_trailing_dot() {
        assert_eq!(add_commas("1234."), "1,234.");
    }

    /// 前後の空白を保ったままカンマを付与・除去すること
    #[test]
    fn test_surrounding_whitespace_is_preserved() {
        assert_eq!(add_commas("  5678  "), "  5,678  ");
        assert_eq!(remove_commas("\t1,234 "), "\t1234 ");
        // 変更が不要な行は空白ごと元のまま返す
        assert!(matches!(add_commas("  123"), Cow::Borrowed(_)));
        assert!(matches!(remove_commas("  1234"), Cow::Borrowed(_)));
        assert_eq!(add_commas("  123\n  1234"), "  123\n  1,234");
    }

    /// 複数行の数値を行ごとに処理し、数値でない行と改行コードを保つようにすること
    #[test]
    fn test_number_multiline() {
        assert_eq!(
            add_commas("1234\r\n-5678.9\nmemo\n\n1,234\n"),
            "1,234\r\n-5,678.9\nmemo\n\n1,234\n"
        );
        assert_eq!(
            remove_commas("1,234\r\n-5,678.9\nmemo 1,234\n\n1234\n"),
            "1234\r\n-5678.9\nmemo 1,234\n\n1234\n"
        );
        // 変更が無ければ元の文字列を返す
        assert!(matches!(add_commas("1,234\nmemo\n"), Cow::Borrowed(_)));
        assert!(matches!(remove_commas("1234\nmemo\n"), Cow::Borrowed(_)));
        // 行内に数値以外が混ざる行は変えない
        assert_eq!(add_commas("1234\n1234 yen"), "1,234\n1234 yen");
    }

    /// `remove_commas`: カンマがない場合は Borrowed を返すようにすること
    #[test]
    fn test_remove_commas_no_comma_returns_borrowed() {
        let input = "1234";
        assert!(matches!(remove_commas(input), Cow::Borrowed(_)));
    }

    /// `remove_commas`: 空文字列は Borrowed を返すようにすること
    #[test]
    fn test_remove_commas_empty() {
        assert!(matches!(remove_commas(""), Cow::Borrowed(_)));
    }
}

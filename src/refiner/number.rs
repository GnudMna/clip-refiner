use std::borrow::Cow;

/// 数値にカンマを付与する
///
/// # Arguments
/// * `text` - カンマを付与する対象の文字列型数値。
///
/// # Returns
/// * `Cow<'_, str>` - 3桁ごとにカンマが付与された文字列。数値として認識できない場合は元の文字列を返す。
pub fn add_commas(text: &str) -> Cow<'_, str> {
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

    // 整数部にカンマを付与
    let mut result = String::new();
    let is_negative = integer_part.starts_with('-');
    let abs_integer = if is_negative {
        &integer_part[1..]
    } else {
        integer_part
    };

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
        format!("{}.{}", formatted_int, decimal_part)
    } else if pure_numeric.contains('.') {
        // 元々小数点が末尾にあった場合
        format!("{}.", formatted_int)
    } else {
        formatted_int
    };

    if final_result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(final_result)
    }
}

/// 数値からカンマを除去する
///
/// # Arguments
/// * `text` - カンマを除去する対象の文字列。
///
/// # Returns
/// * `Cow<'_, str>` - カンマが除去された文字列。数値として認識できない場合は元の文字列を返す。
pub fn remove_commas(text: &str) -> Cow<'_, str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed(text);
    }

    if !is_numeric_input(trimmed) {
        return Cow::Borrowed(text);
    }

    let result = trimmed.replace(',', "");
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 入力が数値(およびカンマ、小数点、マイナス記号)のみで構成されているか判定
///
/// # Arguments
/// * `text` - 判定対象の文字列。
///
/// # Returns
/// * `bool` - 数値入力として妥当な場合は `true`、そうでない場合は `false`。
fn is_numeric_input(text: &str) -> bool {
    let mut has_decimal = false;
    let mut chars = text.chars().peekable();

    // マイナス記号のチェック
    if let Some('-') = chars.peek() {
        chars.next();
    }

    if chars.peek().is_none() {
        return false;
    }

    for c in chars {
        match c {
            '0'..='9' => {}
            ',' => {}
            '.' => {
                if has_decimal {
                    return false; // 小数点が複数ある
                }
                has_decimal = true;
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3桁区切りのカンマを追加する基本的なテスト
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

    /// カンマを除去する基本的なテスト
    #[test]
    fn test_remove_commas() {
        assert_eq!(remove_commas("1,234"), "1234");
        assert_eq!(remove_commas("1,234,567"), "1234567");
        assert_eq!(remove_commas("1,234.56"), "1234.56");
        assert_eq!(remove_commas("-1,234"), "-1234");
        assert_eq!(remove_commas("1,234 yen"), "1,234 yen");
        assert_eq!(remove_commas(""), "");
    }

    /// ゼロに対するカンマ追加のテスト
    #[test]
    fn test_add_commas_zero() {
        assert_eq!(add_commas("0"), "0");
        assert_eq!(add_commas("0.0"), "0.0");
    }

    /// 負の数に対するカンマ追加のテスト
    #[test]
    fn test_add_commas_negative() {
        assert_eq!(add_commas("-1234.56"), "-1,234.56");
        assert_eq!(add_commas("-.5"), "-.5");
    }

    /// 数値判定ロジックのテスト
    /// 数字、カンマ、ピリオド、マイナス記号以外が含まれる場合はfalseとなることを確認
    #[test]
    fn test_is_numeric_input_check() {
        assert!(!is_numeric_input("abc"));
        assert!(!is_numeric_input("12.34.56"));
        assert!(is_numeric_input("-1,234.5"));
    }
}

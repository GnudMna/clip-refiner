/// 数値にカンマを付与する
pub fn add_commas(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_string();
    }

    // 数値、カンマ、小数点以外の文字が含まれているかチェック
    // カンマが含まれていても、最終的に一貫した形式にするために一旦除去して再付与するアプローチを取る
    if !is_numeric_input(trimmed) {
        return text.to_string();
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

    let mut count = 0;
    for c in abs_integer.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }

    let mut formatted_int: String = result.chars().rev().collect();
    if is_negative {
        formatted_int.insert(0, '-');
    }

    if !decimal_part.is_empty() {
        format!("{}.{}", formatted_int, decimal_part)
    } else if pure_numeric.contains('.') {
        // 元々小数点が末尾にあった場合
        format!("{}.", formatted_int)
    } else {
        formatted_int
    }
}

/// 数値からカンマを除去する
pub fn remove_commas(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_string();
    }

    if !is_numeric_input(trimmed) {
        return text.to_string();
    }

    trimmed.replace(',', "")
}

/// 入力が数値(およびカンマ、小数点、マイナス記号)のみで構成されているか判定
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

    #[test]
    fn test_remove_commas() {
        assert_eq!(remove_commas("1,234"), "1234");
        assert_eq!(remove_commas("1,234,567"), "1234567");
        assert_eq!(remove_commas("1,234.56"), "1234.56");
        assert_eq!(remove_commas("-1,234"), "-1234");
        assert_eq!(remove_commas("1,234 yen"), "1,234 yen");
        assert_eq!(remove_commas(""), "");
    }
}

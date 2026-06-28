use std::borrow::Cow;

// ======================================================================
// 識別子分割
// ======================================================================
/// 識別子文字列を単語列へ分割する
///
/// `snake_case` / `kebab-case` / 空白区切り / `camelCase` / `PascalCase` に対応
fn split_identifier(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if trimmed
        .chars()
        .any(|c| c == '_' || c == '-' || c.is_whitespace())
    {
        return split_on_separators(trimmed);
    }

    split_camel_case(trimmed)
}

/// 区切り文字 (`_`, `-`, 空白) で分割する
fn split_on_separators(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for c in input.chars() {
        if c == '_' || c == '-' || c.is_whitespace() {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        words.push(current.to_lowercase());
    }

    words
}

/// `camelCase` / `PascalCase` を単語境界で分割する
fn split_camel_case(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = index > 0 && chars[index - 1].is_lowercase();
            let next_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            if !current.is_empty() && (prev_lower || next_lower) {
                words.push(current.to_lowercase());
                current.clear();
            }
        }
        current.push(c);
    }

    if !current.is_empty() {
        words.push(current.to_lowercase());
    }

    words
}

/// 先頭文字のみ大文字化する
fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result = String::new();
    result.extend(first.to_uppercase());
    result.extend(chars);
    result
}

// ======================================================================
// 行単位変換
// ======================================================================
/// 各行を識別子ケースへ変換する
fn convert_lines<F>(input: &str, convert: F) -> Cow<'_, str>
where
    F: Fn(&str) -> String,
{
    let line_ending = super::utils::detect_line_ending(input);
    let mut changed = false;
    let lines: Vec<String> = input
        .split('\n')
        .map(|line| {
            let body = line.trim_end_matches('\r');
            if body.trim().is_empty() {
                return body.to_string();
            }
            let converted = convert(body);
            if converted != body {
                changed = true;
            }
            converted
        })
        .collect();

    if !changed {
        return Cow::Borrowed(input);
    }

    Cow::Owned(lines.join(line_ending))
}

// ======================================================================
// ケース変換
// ======================================================================
/// `camelCase` へ変換する
///
/// # Arguments
/// * `text` - 変換対象の識別子文字列 (複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列を借用
pub fn to_camel_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        let words = split_identifier(line);
        if words.is_empty() {
            return line.to_string();
        }
        let mut result = words[0].clone();
        for word in words.iter().skip(1) {
            result.push_str(&capitalize_word(word));
        }
        result
    })
}

/// `snake_case` へ変換する
pub fn to_snake_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        let words = split_identifier(line);
        if words.is_empty() {
            return line.to_string();
        }
        words.join("_")
    })
}

/// `PascalCase` へ変換する
pub fn to_pascal_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        let words = split_identifier(line);
        if words.is_empty() {
            return line.to_string();
        }
        words.iter().map(|word| capitalize_word(word)).collect()
    })
}

/// `kebab-case` へ変換する
pub fn to_kebab_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        let words = split_identifier(line);
        if words.is_empty() {
            return line.to_string();
        }
        words.join("-")
    })
}

/// `SCREAMING_SNAKE_CASE` へ変換する
pub fn to_screaming_snake_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        let words = split_identifier(line);
        if words.is_empty() {
            return line.to_string();
        }
        words
            .iter()
            .map(|word| word.to_uppercase())
            .collect::<Vec<_>>()
            .join("_")
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `split_identifier` が各形式を単語列へ分割すること
    #[test]
    fn test_split_identifier() {
        assert_eq!(split_identifier("foo_bar"), vec!["foo", "bar"]);
        assert_eq!(split_identifier("foo-bar"), vec!["foo", "bar"]);
        assert_eq!(split_identifier("fooBar"), vec!["foo", "bar"]);
        assert_eq!(split_identifier("FooBar"), vec!["foo", "bar"]);
        assert_eq!(
            split_identifier("getHTTPResponse"),
            vec!["get", "http", "response"]
        );
        assert_eq!(split_identifier("HTTPResponse"), vec!["http", "response"]);
        assert_eq!(split_identifier("URL"), vec!["url"]);
    }

    /// `to_camel_case` の基本変換
    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("foo_bar"), "fooBar");
        assert_eq!(to_camel_case("foo-bar"), "fooBar");
        assert_eq!(to_camel_case("FooBar"), "fooBar");
        assert_eq!(to_camel_case("getHTTPResponse"), "getHttpResponse");
    }

    /// `to_snake_case` の基本変換
    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("fooBar"), "foo_bar");
        assert_eq!(to_snake_case("FooBar"), "foo_bar");
        assert_eq!(to_snake_case("foo-bar"), "foo_bar");
        assert_eq!(to_snake_case("getHTTPResponse"), "get_http_response");
    }

    /// `to_pascal_case` の基本変換
    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("foo_bar"), "FooBar");
        assert_eq!(to_pascal_case("fooBar"), "FooBar");
        assert_eq!(to_pascal_case("foo-bar"), "FooBar");
    }

    /// `to_kebab_case` の基本変換
    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("fooBar"), "foo-bar");
        assert_eq!(to_kebab_case("foo_bar"), "foo-bar");
        assert_eq!(to_kebab_case("FooBar"), "foo-bar");
    }

    /// `to_screaming_snake_case` の基本変換
    #[test]
    fn test_to_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("fooBar"), "FOO_BAR");
        assert_eq!(to_screaming_snake_case("foo-bar"), "FOO_BAR");
        assert_eq!(to_screaming_snake_case("FooBar"), "FOO_BAR");
    }

    /// 複数行入力を行単位で変換すること
    #[test]
    fn test_convert_multiline() {
        assert_eq!(to_snake_case("fooBar\nhelloWorld"), "foo_bar\nhello_world");
        assert_eq!(to_camel_case("foo_bar\r\nbaz_qux"), "fooBar\r\nbazQux");
    }

    /// 空行はそのまま維持すること
    #[test]
    fn test_convert_preserves_empty_lines() {
        assert_eq!(to_snake_case("fooBar\n\nbazQux"), "foo_bar\n\nbaz_qux");
    }

    /// 変更がない場合は Borrowed を返すこと
    #[test]
    fn test_no_change_returns_borrowed() {
        let input = "foo_bar";
        assert!(matches!(to_snake_case(input), Cow::Borrowed(_)));
    }
}

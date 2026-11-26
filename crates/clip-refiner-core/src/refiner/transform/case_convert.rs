use std::borrow::Cow;

// ======================================================================
// 識別子分割
// ======================================================================
/// 識別子文字列を単語列へ分割する
///
/// `snake_case` / `kebab-case` / 空白区切り / `camelCase` / `PascalCase` に対応
/// 区切り文字で分けた各部分にも `camelCase` の境界を適用する (`myVar_name` → `my`, `var`, `name`)
fn split_identifier(input: &str) -> Vec<String> {
    input
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .flat_map(split_camel_case)
        .collect()
}

/// `camelCase` / `PascalCase` を単語境界で分割する
fn split_camel_case(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = index > 0 && chars[index - 1].is_lowercase();
            // `.` などの記号の直後は語頭であり、境界を追加しない (`foo.Bar` → `foo.bar`)
            let prev_alphanumeric = index > 0 && chars[index - 1].is_alphanumeric();
            let next_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            if !current.is_empty() && (prev_lower || (next_lower && prev_alphanumeric)) {
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

/// 先頭の `_` / `-` を維持して単語列を組み立てる
///
/// `_private` や `--flag` の接頭辞は区切りではなく識別子の一部として残す
fn convert_identifier<F>(line: &str, build: F) -> String
where
    F: Fn(Vec<String>) -> String,
{
    let body = line.trim_start_matches(['_', '-']);
    let prefix = &line[..line.len() - body.len()];
    let words = split_identifier(body);
    if words.is_empty() {
        return line.to_string();
    }
    format!("{prefix}{}", build(words))
}

// ======================================================================
// 行単位変換
// ======================================================================
/// 各行を識別子ケースへ変換する
///
/// 行頭のインデントと行末の空白は変換せず維持する
fn convert_lines<F>(input: &str, convert: F) -> Cow<'_, str>
where
    F: Fn(&str) -> String,
{
    super::utils::process_lines(input, |line| {
        let identifier = line.trim();
        if identifier.is_empty() {
            return None;
        }
        let converted = convert(identifier);
        if converted == identifier {
            return None;
        }
        let indent_len = line.len() - line.trim_start().len();
        let trailing_start = indent_len + identifier.len();
        Some(Cow::Owned(format!(
            "{}{converted}{}",
            &line[..indent_len],
            &line[trailing_start..]
        )))
    })
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
        convert_identifier(line, |words| {
            let mut result = words[0].clone();
            for word in words.iter().skip(1) {
                result.push_str(&capitalize_word(word));
            }
            result
        })
    })
}

/// `snake_case` へ変換する
///
/// # Arguments
/// * `text` - 変換対象の識別子文字列 (複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列を借用
pub fn to_snake_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        convert_identifier(line, |words| words.join("_"))
    })
}

/// `PascalCase` へ変換する
///
/// # Arguments
/// * `text` - 変換対象の識別子文字列 (複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列を借用
pub fn to_pascal_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        convert_identifier(line, |words| {
            words.iter().map(|word| capitalize_word(word)).collect()
        })
    })
}

/// `kebab-case` へ変換する
///
/// # Arguments
/// * `text` - 変換対象の識別子文字列 (複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列を借用
pub fn to_kebab_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        convert_identifier(line, |words| words.join("-"))
    })
}

/// `SCREAMING_SNAKE_CASE` へ変換する
///
/// # Arguments
/// * `text` - 変換対象の識別子文字列 (複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列を借用
pub fn to_screaming_snake_case(text: &str) -> Cow<'_, str> {
    convert_lines(text, |line| {
        convert_identifier(line, |words| {
            words
                .iter()
                .map(|word| word.to_uppercase())
                .collect::<Vec<_>>()
                .join("_")
        })
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
        assert_eq!(split_identifier("HTTP_RESPONSE"), vec!["http", "response"]);
    }

    /// 区切り文字と `camelCase` が混在する識別子を両方の境界で分割すること
    #[test]
    fn test_split_identifier_mixed_separators_and_camel_case() {
        assert_eq!(split_identifier("myVar_name"), vec!["my", "var", "name"]);
        assert_eq!(
            split_identifier("getHTTPResponse_code"),
            vec!["get", "http", "response", "code"]
        );
        assert_eq!(
            split_identifier("fooBar-baz qux"),
            vec!["foo", "bar", "baz", "qux"]
        );
        assert_eq!(to_snake_case("myVar_name"), "my_var_name");
        assert_eq!(
            to_kebab_case("getHTTPResponse_code"),
            "get-http-response-code"
        );
    }

    /// `.` などの記号の直後の大文字で余計な区切りを入れないこと
    #[test]
    fn test_symbol_followed_by_uppercase_has_no_extra_separator() {
        assert_eq!(to_snake_case("foo.bar.Baz"), "foo.bar.baz");
        assert_eq!(to_snake_case("foo.barBaz"), "foo.bar_baz");
        assert_eq!(to_snake_case("foo.HTTPResponse"), "foo.http_response");
        assert_eq!(to_kebab_case("foo.bar.Baz"), "foo.bar.baz");
        assert_eq!(to_screaming_snake_case("foo.bar.Baz"), "FOO.BAR.BAZ");
        assert_eq!(to_camel_case("foo.bar.Baz"), "foo.bar.baz");
        assert_eq!(to_pascal_case("foo.bar.Baz"), "Foo.bar.baz");
    }

    /// `to_camel_case` の基本変換できるようにすること
    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("foo_bar"), "fooBar");
        assert_eq!(to_camel_case("foo-bar"), "fooBar");
        assert_eq!(to_camel_case("FooBar"), "fooBar");
        assert_eq!(to_camel_case("getHTTPResponse"), "getHttpResponse");
    }

    /// `to_snake_case` の基本変換できるようにすること
    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("fooBar"), "foo_bar");
        assert_eq!(to_snake_case("FooBar"), "foo_bar");
        assert_eq!(to_snake_case("foo-bar"), "foo_bar");
        assert_eq!(to_snake_case("getHTTPResponse"), "get_http_response");
    }

    /// `to_pascal_case` の基本変換できるようにすること
    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("foo_bar"), "FooBar");
        assert_eq!(to_pascal_case("fooBar"), "FooBar");
        assert_eq!(to_pascal_case("foo-bar"), "FooBar");
    }

    /// `to_kebab_case` の基本変換できるようにすること
    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("fooBar"), "foo-bar");
        assert_eq!(to_kebab_case("foo_bar"), "foo-bar");
        assert_eq!(to_kebab_case("FooBar"), "foo-bar");
    }

    /// `to_screaming_snake_case` の基本変換できるようにすること
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
        // CRLF と LF が混在していても、行ごとの改行コードを保持する
        assert_eq!(
            to_camel_case("foo_bar\r\nbaz_qux\nhello_world\r\n"),
            "fooBar\r\nbazQux\nhelloWorld\r\n"
        );
    }

    /// 行頭のインデントと行末の空白を維持して変換すること
    #[test]
    fn test_convert_preserves_indent() {
        assert_eq!(to_snake_case("    fooBar"), "    foo_bar");
        assert_eq!(to_camel_case("\tfoo_bar  "), "\tfooBar  ");
        assert_eq!(to_snake_case("\u{3000}fooBar"), "\u{3000}foo_bar");
        assert_eq!(
            to_kebab_case("  fooBar\r\n    bazQux\r\n"),
            "  foo-bar\r\n    baz-qux\r\n"
        );
        // 語の間の空白は区切りとして変換する
        assert_eq!(to_snake_case("  foo bar"), "  foo_bar");
        // 変更がない場合はインデントがあっても Borrowed を返す
        assert!(matches!(to_snake_case("    foo_bar"), Cow::Borrowed(_)));
    }

    /// 空行はそのまま維持すること
    #[test]
    fn test_convert_preserves_empty_lines() {
        assert_eq!(to_snake_case("fooBar\n\nbazQux"), "foo_bar\n\nbaz_qux");
    }

    /// 変更がない場合は Borrowed を返すようにすること
    #[test]
    fn test_no_change_returns_borrowed() {
        let input = "foo_bar";
        assert!(matches!(to_snake_case(input), Cow::Borrowed(_)));
    }

    /// 先頭の `_` / `-` を識別子の一部として維持すること
    #[test]
    fn test_leading_separators_are_preserved() {
        assert_eq!(to_snake_case("_privateFoo"), "_private_foo");
        assert_eq!(to_camel_case("__private_foo"), "__privateFoo");
        assert_eq!(to_kebab_case("--dry_run"), "--dry-run");
        assert_eq!(to_snake_case("--dryRun"), "--dry_run");
        assert_eq!(to_pascal_case("_foo_bar"), "_FooBar");
        assert_eq!(to_screaming_snake_case("_fooBar"), "_FOO_BAR");
        // 区切りのみの入力は変換しない
        assert!(matches!(to_snake_case("___"), Cow::Borrowed(_)));
    }
}

/// 文字列内の特殊文字をバックスラッシュでエスケープする
///
/// # Arguments
/// * `input` - エスケープする文字列。
///
/// # Returns
/// * `String` - エスケープされた文字列。
pub fn escape_string(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// 文字列内のバックスラッシュによるエスケープを解除する
///
/// # Arguments
/// * `input` - アンエスケープする文字列。
///
/// # Returns
/// * `String` - アンエスケープされた文字列。
pub fn unescape_string(input: &str) -> String {
    let mut unescaped = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('b') => {
                    unescaped.push('\x08');
                    chars.next();
                }
                Some('f') => {
                    unescaped.push('\x0c');
                    chars.next();
                }
                Some('n') => {
                    unescaped.push('\n');
                    chars.next();
                }
                Some('r') => {
                    unescaped.push('\r');
                    chars.next();
                }
                Some('t') => {
                    unescaped.push('\t');
                    chars.next();
                }
                Some('\"') => {
                    unescaped.push('\"');
                    chars.next();
                }
                Some('\\') => {
                    unescaped.push('\\');
                    chars.next();
                }
                _ => unescaped.push('\\'),
            }
        } else {
            unescaped.push(c);
        }
    }
    unescaped
}

/// 正規表現のメタ文字をエスケープする
///
/// # Arguments
/// * `input` - エスケープする文字列。
///
/// # Returns
/// * `String` - エスケープされた文字列。
pub fn regex_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '.' | '*' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

/// 正規表現のエスケープを解除する
///
/// # Arguments
/// * `input` - アンエスケープする文字列。
///
/// # Returns
/// * `String` - アンエスケープされた文字列。
pub fn regex_unescape(input: &str) -> String {
    let mut unescaped = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                match next {
                    '.' | '*' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']'
                    | '\\' => {
                        unescaped.push(next);
                        chars.next();
                    }
                    _ => unescaped.push('\\'),
                }
            } else {
                unescaped.push('\\');
            }
        } else {
            unescaped.push(c);
        }
    }
    unescaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(
            escape_string("quote \" and backslash \\"),
            "quote \\\" and backslash \\\\"
        );
        assert_eq!(escape_string("\t\r"), "\\t\\r");
    }

    #[test]
    fn test_unescape_string() {
        assert_eq!(unescape_string("hello\\nworld"), "hello\nworld");
        assert_eq!(
            unescape_string("quote \\\" and backslash \\\\"),
            "quote \" and backslash \\"
        );
        assert_eq!(unescape_string("\\t\\r"), "\t\r");
        assert_eq!(unescape_string("unknown \\z"), "unknown \\z");
    }

    #[test]
    fn test_regex_escape() {
        assert_eq!(regex_escape("hello.world*"), "hello\\.world\\*");
        assert_eq!(regex_escape("[a-z]+"), "\\[a-z\\]\\+");
        assert_eq!(regex_escape("^$| () {}"), "\\^\\$\\| \\(\\)\x20\\{\\}");
    }

    #[test]
    fn test_regex_unescape() {
        assert_eq!(regex_unescape("hello\\.world\\*"), "hello.world*");
        assert_eq!(regex_unescape("\\[a-z\\]\\+"), "[a-z]+");
        assert_eq!(regex_unescape("\\^\\$\\| \\(\\)\x20\\{\\}"), "^$| () {}");
        assert_eq!(regex_unescape("normal text"), "normal text");
        assert_eq!(regex_unescape("escaped \\n"), "escaped \\n"); //  regex unescape shouldn't touch \n
    }
}

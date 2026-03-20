use std::borrow::Cow;

/// テキスト全体から使用されている改行コードを判定する
///
/// テキスト内に `\r\n` が含まれている場合は CRLF ("\r\n") を返し、そうでない場合は LF ("\n") を返します。
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `&str` - 検出された改行コード（"\r\n" または "\n"）。
pub fn detect_line_ending(text: &str) -> &str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// 文字列を改行コードで分割し、各行に対して処理を行う共通ユーティリティ
///
/// 改行形式を自動判別し、各行に対してクロージャ `f` を適用します。
/// 少なくとも1つの行で変更があった場合のみ、新しい文字列を構築して返します。
///
/// # Arguments
/// * `text` - 処理対象の文字列
/// * `f` - 各行に対する処理。変更があった場合は `Some(Cow::Owned)`、なかった場合は `None` または `Some(Cow::Borrowed)` を返す
///
/// # Returns
/// * `Cow<'_, str>` - 少なくとも1行で変更があった場合は `Cow::Owned(結合後のテキスト)` を返し、そうでない場合は `Cow::Borrowed(text)` を返します。
pub fn process_lines<'a, F>(text: &'a str, f: F) -> Cow<'a, str>
where
    F: Fn(&str) -> Option<Cow<'_, str>>,
{
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let line_ending = detect_line_ending(text);
    let mut changed = false;
    let mut result = String::with_capacity(text.len());

    for (i, line) in text.split(line_ending).enumerate() {
        if i > 0 {
            result.push_str(line_ending);
        }

        match f(line) {
            Some(Cow::Owned(processed)) => {
                result.push_str(&processed);
                changed = true;
            }
            Some(Cow::Borrowed(processed)) => {
                if processed != line {
                    changed = true;
                }
                result.push_str(processed);
            }
            None => {
                result.push_str(line);
            }
        }
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_line_ending() {
        assert_eq!(detect_line_ending("line1\nline2"), "\n");
        assert_eq!(detect_line_ending("line1\r\nline2"), "\r\n");
        assert_eq!(detect_line_ending("no newline"), "\n");
    }

    #[test]
    fn test_process_lines() {
        let input = "a\nb\nc";
        let result = process_lines(input, |line| {
            if line == "b" {
                Some(Cow::Owned("B".to_string()))
            } else {
                None
            }
        });
        assert_eq!(result, "a\nB\nc");

        let no_change = process_lines(input, |_| None);
        assert!(matches!(no_change, Cow::Borrowed(_)));
        assert_eq!(no_change, input);
    }
}

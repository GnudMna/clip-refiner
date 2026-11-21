use std::borrow::Cow;

// ======================================================================
// 改行コード検出
// ======================================================================
/// テキスト全体から使用されている改行コードを判定する
///
/// テキスト内に `\r\n` が含まれている場合は CRLF ("\r\n") を返し、そうでない場合は LF ("\n") を返す
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `&str` - 検出された改行コード("\r\n" または "\n")
pub fn detect_line_ending(text: &str) -> &str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

// ======================================================================
// 行処理ユーティリティ
// ======================================================================
/// 行を改行コードで結合する
///
/// 元のテキストが改行で終わっていた場合は、末尾に改行を 1 つ付けて元の終端を保つ
/// 最終行が空行の場合も末尾に改行を 1 つ足す。結合結果の末尾の改行は `str::lines` で
/// 行の終端とみなされるため、足さないと再度行に分けたときに空行が 1 つ消える
///
/// # Arguments
/// * `lines` - 結合する行 (改行を含まない)
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `trailing_newline` - 元のテキストが改行で終わっていた場合は `true`
///
/// # Returns
/// * `String` - 結合後のテキスト。行が無い場合は空文字列
pub fn join_lines(lines: &[&str], line_ending: &str, trailing_newline: bool) -> String {
    let mut result = lines.join(line_ending);
    if !lines.is_empty() && (trailing_newline || lines.last().is_some_and(|line| line.is_empty())) {
        result.push_str(line_ending);
    }
    result
}

/// 文字列を改行コードで分割し、各行に対して処理を行う共通ユーティリティ
///
/// `\n` で分割して各行にクロージャ `f` を適用する
/// 行末の改行コード (`\n` または `\r\n`) は行ごとに判別して保持するため、CRLF と LF が混在していても行は混ざらない
/// 少なくとも1つの行で変更があった場合のみ、新しい文字列を構築して返す
///
/// # Arguments
/// * `text` - 処理対象の文字列
/// * `f` - 各行に対する処理 (改行コードを除いた行が渡される)。変更があった場合は `Some(Cow::Owned)`、なかった場合は `None` または `Some(Cow::Borrowed)` を返す
///
/// # Returns
/// * `Cow<'_, str>` - 少なくとも1行で変更があった場合は `Cow::Owned(結合後のテキスト)` を返し、そうでない場合は `Cow::Borrowed(text)` を返す
pub fn process_lines<F>(text: &str, f: F) -> Cow<'_, str>
where
    F: Fn(&str) -> Option<Cow<'_, str>>,
{
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let mut changed = false;
    let mut result = String::with_capacity(text.len());
    let last_index = text.matches('\n').count();

    for (i, segment) in text.split('\n').enumerate() {
        // 末尾以外の行は `\n` で終わる。直前の `\r` は改行コードの一部として分けて保持する
        let (line, line_ending) = if i < last_index {
            match segment.strip_suffix('\r') {
                Some(line) => (line, "\r\n"),
                None => (segment, "\n"),
            }
        } else {
            (segment, "")
        };

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
        result.push_str(line_ending);
    }

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(text)
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// CRLF / LF の改行コード判定できるようにすること
    #[test]
    fn test_detect_line_ending() {
        // LF
        assert_eq!(detect_line_ending("line1\nline2"), "\n");
        // CRLF
        assert_eq!(detect_line_ending("line1\r\nline2"), "\r\n");
        // 改行なし
        assert_eq!(detect_line_ending("no newline"), "\n");
    }

    /// 元が改行終わり、または最終行が空行のときだけ末尾に改行を足して結合すること
    #[test]
    fn test_join_lines() {
        assert_eq!(join_lines(&["a", "b"], "\n", false), "a\nb");
        // 元が改行で終わっていれば、改行を 1 つだけ付ける
        assert_eq!(join_lines(&["a", "b"], "\n", true), "a\nb\n");
        // 末尾の空行を `lines` で数え直しても残るようにする
        assert_eq!(join_lines(&["a", ""], "\r\n", false), "a\r\n\r\n");
        assert_eq!(join_lines(&["a", ""], "\r\n", true), "a\r\n\r\n");
        assert_eq!(join_lines(&["a", ""], "\r\n", false).lines().count(), 2);
        assert_eq!(join_lines(&[""], "\n", false), "\n");
        // 行が無ければ、改行も付けない
        assert_eq!(join_lines(&[], "\n", true), "");
    }

    /// `process_lines` の行単位変換と Borrowed 返却できるようにすること
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

    /// CRLF と LF が混在していても行ごとに処理し、改行コードを保持すること
    #[test]
    fn test_process_lines_mixed_line_endings() {
        let upper = |text| process_lines(text, |line| Some(Cow::Owned(line.to_uppercase())));
        assert_eq!(upper("a\r\nb\nc\r\n"), "A\r\nB\nC\r\n");
        assert_eq!(upper("a\nb\r\nc"), "A\nB\r\nC");
        // 改行を含まない行は 1 行として渡されること
        let seen = std::cell::RefCell::new(Vec::new());
        let _ = process_lines("x\r\ny\nz", |line| {
            seen.borrow_mut().push(line.to_string());
            None
        });
        assert_eq!(seen.into_inner(), ["x", "y", "z"]);
    }
}

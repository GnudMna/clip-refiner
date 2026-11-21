use std::borrow::Cow;

// ======================================================================
// 行処理ユーティリティ
// ======================================================================
/// テキストを行と、その行末の改行コードの組に分ける
///
/// 改行コードは行ごとに判別するため、CRLF と LF が混在していても行は混ざらない
/// 最終行が改行で終わっていない場合、その改行コードは空文字列になる
///
/// # Arguments
/// * `text` - 分割する対象のテキスト
///
/// # Returns
/// * `Vec<(&str, &str)>` - (改行を除いた行, 行末の改行コード) の組。空文字列は空の `Vec`
pub fn split_line_endings(text: &str) -> Vec<(&str, &str)> {
    text.split_inclusive('\n')
        .map(|segment| {
            if let Some(line) = segment.strip_suffix("\r\n") {
                (line, "\r\n")
            } else if let Some(line) = segment.strip_suffix('\n') {
                (line, "\n")
            } else {
                (segment, "")
            }
        })
        .collect()
}

/// 行を、それぞれの改行コードで結合する
///
/// 元のテキストが改行で終わっていた場合は、末尾に改行を 1 つ付けて元の終端を保つ
/// 最終行が空行の場合も末尾に改行を 1 つ足す。結合結果の末尾の改行は `str::lines` で
/// 行の終端とみなされるため、足さないと再度行に分けたときに空行が 1 つ消える
/// 足す改行や、改行コードが空の行の区切りには、組の中で最後に現れる改行コードを使う
///
/// # Arguments
/// * `lines` - 結合する (改行を含まない行, その行末の改行コード) の組
/// * `trailing_newline` - 元のテキストが改行で終わっていた場合は `true`
///
/// # Returns
/// * `String` - 結合後のテキスト。行が無い場合は空文字列
pub fn join_lines(lines: &[(&str, &str)], trailing_newline: bool) -> String {
    let Some(((last_line, last_ending), init)) = lines.split_last() else {
        return String::new();
    };
    let fallback = lines
        .iter()
        .rev()
        .map(|(_, ending)| *ending)
        .find(|ending| !ending.is_empty())
        .unwrap_or("\n");
    let push_ending = |result: &mut String, ending: &str| {
        result.push_str(if ending.is_empty() { fallback } else { ending });
    };

    let mut result = String::new();
    for (line, ending) in init {
        result.push_str(line);
        push_ending(&mut result, ending);
    }
    result.push_str(last_line);
    if trailing_newline || last_line.is_empty() {
        push_ending(&mut result, last_ending);
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
// BOM
// ======================================================================
/// 先頭の BOM (`U+FEFF`) を取り除く
///
/// BOM 付きでコピーされた JSON / YAML をパーサへ渡す前に使う
///
/// # Arguments
/// * `text` - 対象の文字列
///
/// # Returns
/// * `&str` - 先頭の BOM を除いた文字列。BOM が無い場合は `text` のまま
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 先頭の BOM だけを取り除くこと
    #[test]
    fn test_strip_bom() {
        assert_eq!(strip_bom("\u{FEFF}{}"), "{}");
        assert_eq!(strip_bom("{}"), "{}");
        // 先頭以外の BOM は残す
        assert_eq!(strip_bom("a\u{FEFF}"), "a\u{FEFF}");
        assert_eq!(strip_bom("\u{FEFF}\u{FEFF}a"), "\u{FEFF}a");
    }

    /// 行ごとに改行コードを判別して分けるようにすること
    #[test]
    fn test_split_line_endings() {
        assert_eq!(
            split_line_endings("a\r\nb\nc\r\n"),
            [("a", "\r\n"), ("b", "\n"), ("c", "\r\n")]
        );
        // 最終行が改行で終わっていなければ、改行コードは空
        assert_eq!(split_line_endings("a\nb"), [("a", "\n"), ("b", "")]);
        assert_eq!(split_line_endings("\n"), [("", "\n")]);
        assert!(split_line_endings("").is_empty());
    }

    /// 元が改行終わり、または最終行が空行のときだけ末尾に改行を足して結合すること
    #[test]
    fn test_join_lines() {
        assert_eq!(join_lines(&[("a", "\n"), ("b", "")], false), "a\nb");
        // 元が改行で終わっていれば、その行の改行コードを付ける
        assert_eq!(join_lines(&[("a", "\n"), ("b", "\n")], true), "a\nb\n");
        // 末尾の空行を `lines` で数え直しても残るようにする
        assert_eq!(join_lines(&[("a", "\r\n"), ("", "")], false), "a\r\n\r\n");
        assert_eq!(
            join_lines(&[("a", "\r\n"), ("", "\r\n")], true),
            "a\r\n\r\n"
        );
        assert_eq!(
            join_lines(&[("a", "\r\n"), ("", "")], false)
                .lines()
                .count(),
            2
        );
        // 行ごとの改行コードをそのまま使う
        assert_eq!(
            join_lines(&[("a", "\r\n"), ("b", "\n"), ("c", "\r\n")], true),
            "a\r\nb\nc\r\n"
        );
        // 元が改行終わりでなければ、最後の行の改行コードは付けない
        assert_eq!(join_lines(&[("a", "\r\n"), ("b", "\n")], false), "a\r\nb");
        assert_eq!(join_lines(&[("", "")], false), "\n");
        // 行が無ければ、改行も付けない
        assert_eq!(join_lines(&[], true), "");
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

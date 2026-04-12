use std::borrow::Cow;

use csv::{ReaderBuilder, WriterBuilder};
// ======================================================================
// 並び替え
// ======================================================================
/// 行またはCSVレコード単位でテキストを並び替える
///
/// テキストの内容（プレーンテキストの行、またはCSV形式のレコード）を自動判別し、
/// アルファベット順（大文字小文字無視）でソートします。
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `descending` - 降順にする場合は `true`、昇順にする場合は `false`
///
/// # Returns
/// * `Cow<'_, str>` - 並び替え後のテキスト。変更がない場合は元の文字列への参照を返します。
pub fn sort_lines(text: &str, descending: bool) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let line_ending = super::utils::detect_line_ending(text);

    if is_likely_csv(text) {
        sort_csv_records(text, line_ending, descending)
    } else {
        sort_plain_lines(text, line_ending, descending)
    }
}

// ======================================================================
// 行削除
// ======================================================================
/// テキストから空行を削除する
///
/// 改行のみの行、または空白文字（スペース、タブなど）のみの行を取り除きます。
/// 元の改行形式（LF または CRLF）は維持されます。
///
/// # Arguments
/// * `text` - 処理対象のテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 空行が削除されたテキスト。変更がない場合は元の文字列への参照を返します。
pub fn remove_empty_lines(text: &str) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let line_ending = super::utils::detect_line_ending(text);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let result = lines.join(line_ending);
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 重複行を削除する（出現順序を維持する）
///
/// テキスト内の重複する行を特定し、最初に出現した行のみを残して他を削除します。
/// セット（HashSet）を使用して効率的に重複を判定します。
///
/// # Arguments
/// * `text` - 処理対象のテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 重複行が削除されたテキスト。変更がない場合は元の文字列への参照を返します。
pub fn remove_duplicate_lines(text: &str) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let line_ending = super::utils::detect_line_ending(text);
    let mut seen = std::collections::HashSet::new();
    let mut lines = Vec::new();

    for line in text.lines() {
        if seen.insert(line) {
            lines.push(line);
        }
    }

    let result = lines.join(line_ending);
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

// ======================================================================
// CSV 判定
// ======================================================================
/// テキストがCSVである可能性が高いか判定する
///
/// カンマ区切りかつ、複数行にわたってカラム数が一致するかを簡易的に検証します。
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `bool` - CSVとみなせる場合は `true`、そうでない場合は `false`。
fn is_likely_csv(text: &str) -> bool {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut records = rdr.records();
    if let Some(Ok(first)) = records.next() {
        // カラムが2つ以上あればCSVとみなす
        if first.len() > 1 {
            // さらに数行チェックしてカラム数が一致するか見る(簡易的な検証)
            if let Some(Ok(second)) = records.next() {
                return first.len() == second.len();
            }
            return true;
        }
    }
    false
}

// ======================================================================
// ソートユーティリティ
// ======================================================================
/// CSVレコードとしてテキストを並び替える
///
/// レコード全体をカンマで結合した文字列に基づいてソートを行います。
/// クォートで囲まれた改行を含むレコードも正しく処理します。
///
/// # Arguments
/// * `text` - 並び替える対象のCSVテキスト
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `descending` - 降順にする場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - レコード単位で並び替えられたCSVテキスト。
fn sort_csv_records<'a>(text: &'a str, line_ending: &str, descending: bool) -> Cow<'a, str> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut records: Vec<Vec<String>> = rdr
        .records()
        .filter_map(|r| r.ok())
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();

    // 全体の内容でソート(大文字小文字無視)
    records.sort_by(|a, b| {
        let sa = a.join(",");
        let sb = b.join(",");
        if descending {
            sb.to_lowercase().cmp(&sa.to_lowercase())
        } else {
            sa.to_lowercase().cmp(&sb.to_lowercase())
        }
    });

    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .terminator(if line_ending == "\r\n" {
            csv::Terminator::CRLF
        } else {
            csv::Terminator::Any(b'\n')
        })
        .from_writer(Vec::new());

    for record in records {
        let _ = wtr.write_record(&record);
    }

    let bytes = match wtr.into_inner() {
        Ok(b) => b,
        Err(_) => return Cow::Borrowed(text),
    };
    let result = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return Cow::Borrowed(text),
    };
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 単純なテキスト行として並び替える
///
/// 各行を文字列として比較し、ソートを行います。比較時は大文字小文字を区別しません。
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `descending` - 降順にする場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - 行単位で並び替えられたテキスト。
fn sort_plain_lines<'a>(text: &'a str, line_ending: &str, descending: bool) -> Cow<'a, str> {
    let mut lines: Vec<&str> = text.lines().collect();
    if descending {
        lines.sort_by_key(|b| std::cmp::Reverse(b.to_lowercase()));
    } else {
        lines.sort_by_key(|a| a.to_lowercase());
    }
    let result = lines.join(line_ending);
    if result == text {
        Cow::Borrowed(text)
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

    #[test]
    fn test_sort_plain_lines() {
        let input = "banana\nApple\ncherry";
        let expected = "Apple\nbanana\ncherry";
        assert_eq!(sort_lines(input, false), expected);
    }

    #[test]
    fn test_sort_plain_lines_descending() {
        let input = "banana\nApple\ncherry";
        let expected = "cherry\nbanana\nApple";
        assert_eq!(sort_lines(input, true), expected);
    }

    #[test]
    fn test_sort_lines_preserve_crlf() {
        let input = "banana\r\nApple\r\ncherry";
        let expected = "Apple\r\nbanana\r\ncherry";
        assert_eq!(sort_lines(input, false), expected);
    }

    #[test]
    fn test_sort_csv_with_newlines() {
        let input = "z,\"cell\nwith\nnewline\",3\na,\"simple\",1";
        // csv crate handles quotes and escapes.
        // Sorting will put 'a' row first.
        let output = sort_lines(input, false);
        assert!(output.starts_with("a,simple,1"));
        assert!(output.contains("z,\"cell\nwith\nnewline\",3"));
    }

    #[test]
    fn test_remove_empty_lines() {
        let input = "line1\n\n  \nline2\n\t\nline3";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_duplicate_lines() {
        let input = "line1\nline2\nline1\nline3\nline2";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_duplicate_lines(input), expected);
    }
}

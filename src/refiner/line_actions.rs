use csv::{ReaderBuilder, WriterBuilder};

/// 行またはCSVレコード単位でテキストを並び替える
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト。
/// * `descending` - 降順にする場合は `true`。
///
/// # Returns
/// * `String` - 並び替え後のテキスト。
pub fn sort_lines(text: &str, descending: bool) -> String {
    if text.is_empty() {
        return String::new();
    }

    let line_ending = super::utils::detect_line_ending(text);

    if is_likely_csv(text) {
        sort_csv_records(text, line_ending, descending)
    } else {
        sort_plain_lines(text, line_ending, descending)
    }
}

/// 空行を削除する
///
/// # Arguments
/// * `text` - 処理対象のテキスト。
///
/// # Returns
/// * `String` - 空行が削除されたテキスト。
pub fn remove_empty_lines(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let line_ending = super::utils::detect_line_ending(text);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines.join(line_ending)
}

/// 重複行を削除する（順序を維持する）
///
/// # Arguments
/// * `text` - 処理対象のテキスト。
///
/// # Returns
/// * `String` - 重複行が削除されたテキスト。
pub fn remove_duplicate_lines(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let line_ending = super::utils::detect_line_ending(text);
    let mut seen = std::collections::HashSet::new();
    let mut lines = Vec::new();

    for line in text.lines() {
        if seen.insert(line) {
            lines.push(line);
        }
    }

    lines.join(line_ending)
}

/// CSVである可能性が高いか判定する
///
/// # Arguments
/// * `text` - 判定対象のテキスト。
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

/// CSVレコードとして並び替える
///
/// # Arguments
/// * `text` - 並び替える対象のCSVテキスト。
/// * `line_ending` - 使用する改行コード。
/// * `descending` - 降順にする場合は `true`。
///
/// # Returns
/// * `String` - レコード単位で並び替えられたCSVテキスト。
fn sort_csv_records(text: &str, line_ending: &str, descending: bool) -> String {
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

    String::from_utf8(wtr.into_inner().unwrap_or_default()).unwrap_or_default()
}

/// 単純な行として並び替える
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト。
/// * `line_ending` - 使用する改行コード。
/// * `descending` - 降順にする場合は `true`。
///
/// # Returns
/// * `String` - 行単位で並び替えられたテキスト。
fn sort_plain_lines(text: &str, line_ending: &str, descending: bool) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if descending {
        lines.sort_by(|a, b| b.to_lowercase().cmp(&a.to_lowercase()));
    } else {
        lines.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    }
    lines.join(line_ending)
}

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

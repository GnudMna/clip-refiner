use std::borrow::Cow;

use csv::{ReaderBuilder, WriterBuilder};

// ======================================================================
// 並び替え
// ======================================================================
/// 行またはCSVレコード単位でテキストを並び替える
///
/// テキストの内容(プレーンテキストの行、またはCSV形式のレコード)を自動判別し、
/// アルファベット順(大文字小文字無視)でソートする
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `descending` - 降順にする場合は `true`、昇順にする場合は `false`
///
/// # Returns
/// * `Cow<'_, str>` - 並び替え後のテキスト。変更がない場合は元の文字列への参照を返す。
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
/// 改行のみの行、または空白文字(スペース、タブなど)のみの行を取り除く
/// 元の改行形式(LF または CRLF)は維持される
///
/// # Arguments
/// * `text` - 処理対象のテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 空行が削除されたテキスト。変更がない場合は元の文字列への参照を返す。
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

/// 重複行を削除する(出現順序を維持する)
///
/// テキスト内の重複する行を特定し、最初に出現した行のみを残して他を削除する
/// セット(HashSet)を使用して効率的に重複を判定する
///
/// # Arguments
/// * `text` - 処理対象のテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 重複行が削除されたテキスト。変更がない場合は元の文字列への参照を返す。
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
/// 以下の条件をすべて満たす場合に CSV と判定する
/// 1. カラム数が 2 以上
/// 2. 行数が 2 以上(1行のみでは断定しない)
/// 3. すべての行でカラム数が一致する(不一致・パースエラーは`false`)
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `bool` - CSVとみなせる場合は`true`、そうでない場合は`false`
fn is_likely_csv(text: &str) -> bool {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut records = rdr.records();

    // 1行目取得・パース失敗はCSVでない
    let Some(Ok(first)) = records.next() else {
        return false;
    };

    // カラムが2つ未満はCSVでない
    let col_count = first.len();
    if col_count < 2 {
        return false;
    }

    // 2行目が必須: 1行だけではCSVと断定しない
    // パースエラー(列数不一致含む)はCSVでないと判断する
    let Some(Ok(second)) = records.next() else {
        return false;
    };
    if second.len() != col_count {
        return false;
    }

    // 残りの行もカラム数が一貫しているか確認(最大5行まで)
    for record in records.take(5) {
        match record {
            Ok(r) if r.len() == col_count => {}
            _ => return false,
        }
    }

    true
}

// ======================================================================
// ソートユーティリティ
// ======================================================================
/// CSVレコードとしてテキストを並び替える
///
/// レコード全体をカンマで結合した文字列に基づいてソートを行う
/// クォートで囲まれた改行を含むレコードも正しく処理する
///
/// # Arguments
/// * `text` - 並び替える対象のCSVテキスト
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `descending` - 降順にする場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - レコード単位で並び替えられたCSVテキスト
fn sort_csv_records<'a>(text: &'a str, line_ending: &str, descending: bool) -> Cow<'a, str> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut records: Vec<Vec<String>> = rdr
        .records()
        .filter_map(std::result::Result::ok)
        .map(|r| r.iter().map(std::string::ToString::to_string).collect())
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

    let Ok(bytes) = wtr.into_inner() else {
        return Cow::Borrowed(text);
    };
    let Ok(result) = String::from_utf8(bytes) else {
        return Cow::Borrowed(text);
    };
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 単純なテキスト行として並び替える
///
/// 各行を文字列として比較し、ソートを行う。比較時は大文字小文字を区別しない。
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `descending` - 降順にする場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - 行単位で並び替えられたテキスト
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

    /// プレーンテキスト行の昇順ソート
    #[test]
    fn test_sort_plain_lines() {
        let input = "banana\nApple\ncherry";
        let expected = "Apple\nbanana\ncherry";
        assert_eq!(sort_lines(input, false), expected);
    }

    /// プレーンテキスト行の降順ソート
    #[test]
    fn test_sort_plain_lines_descending() {
        let input = "banana\nApple\ncherry";
        let expected = "cherry\nbanana\nApple";
        assert_eq!(sort_lines(input, true), expected);
    }

    /// CRLF 入力でソート後も CRLF を保持すること
    #[test]
    fn test_sort_lines_preserve_crlf() {
        let input = "banana\r\nApple\r\ncherry";
        let expected = "Apple\r\nbanana\r\ncherry";
        assert_eq!(sort_lines(input, false), expected);
    }

    /// クォート内改行を含む CSV のソート
    #[test]
    fn test_sort_csv_with_newlines() {
        let input = "z,\"cell\nwith\nnewline\",3\na,\"simple\",1";
        // csv クレートがクォート内改行を処理する
        // 'a' 行が先頭になること
        let output = sort_lines(input, false);
        assert!(output.starts_with("a,simple,1"));
        assert!(output.contains("z,\"cell\nwith\nnewline\",3"));
    }

    /// 空行削除の基本動作
    #[test]
    fn test_remove_empty_lines() {
        let input = "line1\n\n  \nline2\n\t\nline3";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_empty_lines(input), expected);
    }

    /// 重複行削除(出現順序維持)
    #[test]
    fn test_remove_duplicate_lines() {
        let input = "line1\nline2\nline1\nline3\nline2";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_duplicate_lines(input), expected);
    }

    /// 空文字列は変更なしで返ること
    #[test]
    fn test_sort_empty() {
        assert!(matches!(sort_lines("", false), Cow::Borrowed(_)));
        assert!(matches!(remove_empty_lines(""), Cow::Borrowed(_)));
        assert!(matches!(remove_duplicate_lines(""), Cow::Borrowed(_)));
    }

    /// 既にソート済みの場合は Borrowed を返す (変更なし)
    #[test]
    fn test_sort_already_sorted_returns_borrowed() {
        let input = "apple\nbanana\ncherry";
        assert!(matches!(sort_lines(input, false), Cow::Borrowed(_)));
    }

    /// 1行のみの場合もソートが機能すること
    #[test]
    fn test_sort_single_line() {
        let input = "single";
        assert_eq!(sort_lines(input, false), "single");
    }

    /// `is_likely_csv`: カンマ区切りで複数列かつ行数が一致すればCSV判定
    #[test]
    fn test_is_likely_csv_true() {
        // 2列 × 2行
        assert!(is_likely_csv("a,b\nc,d"));
        // 3列 × 2行
        assert!(is_likely_csv("1,2,3\n4,5,6"));
        // 2列 × 3行
        assert!(is_likely_csv("banana,2\napple,1\ncherry,3"));
    }

    /// `is_likely_csv`: 1列しかない場合はCSVでない
    #[test]
    fn test_is_likely_csv_false_single_column() {
        assert!(!is_likely_csv("apple\nbanana\ncherry"));
    }

    /// `is_likely_csv`: 1行だけではCSVと断定しない
    #[test]
    fn test_is_likely_csv_false_single_row() {
        assert!(!is_likely_csv("a,b,c"));
        assert!(!is_likely_csv("hello,world"));
    }

    /// `is_likely_csv`: 列数が揃っていない場合はCSVでない
    #[test]
    fn test_is_likely_csv_false_inconsistent_columns() {
        assert!(!is_likely_csv("a,b,c\nd,e"));
        assert!(!is_likely_csv("1,2\n3,4,5"));
    }

    /// CSVレコードの昇順ソート
    #[test]
    fn test_sort_csv_records_asc() {
        let input = "banana,2\napple,1\ncherry,3";
        let output = sort_lines(input, false);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "apple,1");
        assert_eq!(lines[1], "banana,2");
        assert_eq!(lines[2], "cherry,3");
    }

    /// CSVレコードの降順ソート
    #[test]
    fn test_sort_csv_records_desc() {
        let input = "banana,2\napple,1\ncherry,3";
        let output = sort_lines(input, true);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "cherry,3");
        assert_eq!(lines[1], "banana,2");
        assert_eq!(lines[2], "apple,1");
    }

    /// CRLF 入力で空行削除しても CRLF が保持されること
    #[test]
    fn test_remove_empty_lines_crlf() {
        let input = "line1\r\n\r\nline2\r\n";
        let output = remove_empty_lines(input);
        assert_eq!(output, "line1\r\nline2");
    }

    /// CRLF 入力で重複行削除しても CRLF が保持されること
    #[test]
    fn test_remove_duplicate_lines_crlf() {
        let input = "line1\r\nline2\r\nline1\r\n";
        let output = remove_duplicate_lines(input);
        assert_eq!(output, "line1\r\nline2");
    }
}

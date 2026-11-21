use std::borrow::Cow;

use csv::ReaderBuilder;

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

    // クォート内改行を含むレコードが無い場合や、CSV 判定の範囲外に解析できないレコードがある場合は行単位で並び替える
    // csv クレートは単独の CR もレコードの区切りとみなし、行単位の区切り (LF / CRLF) とずれるため、
    // 単独の CR を含むテキストも行単位で並び替える
    if !has_lone_cr(text)
        && is_likely_csv(text)
        && let Some(sorted) = sort_csv_records(text, line_ending, descending)
    {
        return sorted;
    }
    sort_plain_lines(text, line_ending, descending)
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

    let result = super::utils::join_lines(&lines, line_ending);
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

/// LF が続かない単独の CR を含むか判定する
fn has_lone_cr(text: &str) -> bool {
    text.match_indices('\r')
        .any(|(i, _)| text.as_bytes().get(i + 1) != Some(&b'\n'))
}

// ======================================================================
// ソートユーティリティ
// ======================================================================
/// CSVレコードとしてテキストを並び替える
///
/// クォートで囲まれた改行を含むレコードを 1 つの単位として扱い、レコード全体をカンマで結合した文字列に基づいてソートする
/// 出力には元のレコードの文字列をそのまま使い、空行も行単位の並び替えと同じく 1 行として残す
/// クォート内改行を含むレコードが無い場合や、解析に失敗したレコードが 1 件でもある場合は `None` を返す
///
/// # Arguments
/// * `text` - 並び替える対象のCSVテキスト
/// * `line_ending` - 使用する改行コード ("\n" または "\r\n")
/// * `descending` - 降順にする場合は `true`
///
/// # Returns
/// * `Option<Cow<'_, str>>` - レコード単位で並び替えられたCSVテキスト。
///   行単位で並び替えるべき場合は `None`
fn sort_csv_records<'a>(
    text: &'a str,
    line_ending: &str,
    descending: bool,
) -> Option<Cow<'a, str>> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    // (比較キー, 元の文字列)
    let mut entries: Vec<(String, &str)> = Vec::new();
    let mut has_multiline_record = false;
    // 直前のレコード本体の終端。csv クレートは空行を読み飛ばすため、レコード間の区間から空行を数え直す
    let mut prev_end: Option<usize> = None;
    let mut record = csv::StringRecord::new();

    // 列数の不一致も解析エラーになるため、1 件でも失敗すれば CSV として扱わない
    while rdr.read_record(&mut record).ok()? {
        let pos = usize::try_from(record.position()?.byte()).ok()?;
        let end = usize::try_from(rdr.position().byte()).ok()?;
        // レコードの位置は前後の改行や空行を含むことがあるため、本体だけを切り出す
        let with_head = text[pos..end].trim_end_matches(['\r', '\n']);
        let raw = with_head.trim_start_matches(['\r', '\n']);
        let start = pos + (with_head.len() - raw.len());
        push_blank_lines(
            &mut entries,
            &text[prev_end.unwrap_or(0)..start],
            prev_end.is_some(),
        );

        has_multiline_record |= raw.contains(['\r', '\n']);
        entries.push((
            record.iter().collect::<Vec<_>>().join(",").to_lowercase(),
            raw,
        ));
        prev_end = Some(start + raw.len());
    }
    if !has_multiline_record {
        return None;
    }
    push_blank_lines(
        &mut entries,
        &text[prev_end.unwrap_or(0)..],
        prev_end.is_some(),
    );

    if descending {
        entries.sort_by(|a, b| b.0.cmp(&a.0));
    } else {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let lines: Vec<&str> = entries.iter().map(|(_, raw)| *raw).collect();
    let result = super::utils::join_lines(&lines, line_ending);
    if result == text {
        Some(Cow::Borrowed(text))
    } else {
        Some(Cow::Owned(result))
    }
}

/// レコード間の区間に含まれる空行を、比較キーが空のエントリとして追加する
///
/// # Arguments
/// * `entries` - 追加先の (比較キー, 元の文字列) の一覧
/// * `gap` - 前のレコード本体の終端から次のレコードの先頭までの文字列
/// * `after_record` - `gap` の先頭が直前のレコードの改行である場合は `true`
fn push_blank_lines(entries: &mut Vec<(String, &str)>, gap: &str, after_record: bool) {
    let mut count = gap.lines().count();
    if after_record {
        count = count.saturating_sub(1);
    }
    entries.extend(std::iter::repeat_n((String::new(), ""), count));
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
    let result = super::utils::join_lines(&lines, line_ending);
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

    /// プレーンテキスト行の昇順ソートできるようにすること
    #[test]
    fn test_sort_plain_lines() {
        let input = "banana\nApple\ncherry";
        let expected = "Apple\nbanana\ncherry";
        assert_eq!(sort_lines(input, false), expected);
    }

    /// プレーンテキスト行の降順ソートできるようにすること
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

    /// クォート内改行を含む CSV のソートできるようにすること
    #[test]
    fn test_sort_csv_with_newlines() {
        let input = "z,\"cell\nwith\nnewline\",3\na,\"simple\",1";
        // クォート内改行を含むレコードを 1 単位とし、元のクォートを保ったまま並び替える
        let output = sort_lines(input, false);
        assert_eq!(output, "a,\"simple\",1\nz,\"cell\nwith\nnewline\",3");
    }

    /// クォート内改行を含む CSV でも空行を残し、CRLF を保持すること
    #[test]
    fn test_sort_csv_with_newlines_keeps_blank_lines_and_crlf() {
        let input = "z,\"x\r\ny\"\r\n\r\na,1\r\n";
        assert_eq!(sort_lines(input, false), "\r\na,1\r\nz,\"x\r\ny\"");
        // 空行が最後に来る場合は、末尾の改行が行の終端とみなされて消えないよう改行を 1 つ足す
        assert_eq!(sort_lines(input, true), "z,\"x\r\ny\"\r\na,1\r\n\r\n");

        // 先頭・途中・末尾の空行を行単位の並び替えと同じ数だけ残す
        let input = "\n\nz,\"a\nb\"\n\n\nc,d\n\n";
        assert_eq!(sort_lines(input, false), "\n\n\n\n\nc,d\nz,\"a\nb\"");
    }

    /// クォート内改行が無ければ、CSV とみなせても元の行の文字列のまま並び替えること
    #[test]
    fn test_sort_csv_like_lines_keeps_original_text() {
        let input = "log(\"y\", 2)\nlog(\"x\", 1)";
        assert!(is_likely_csv(input));
        assert_eq!(sort_lines(input, false), "log(\"x\", 1)\nlog(\"y\", 2)");
    }

    /// クォート内改行が無ければ、CSV とみなせても空行を残し末尾に改行を付けないこと
    #[test]
    fn test_sort_csv_like_lines_keeps_blank_lines() {
        assert_eq!(sort_lines("b,1\n\na,2\n", false), "\na,2\nb,1");
    }

    /// クォート内に単独の CR を含む CSV でも、CR を改行に変えず行単位で並び替えること
    #[test]
    fn test_sort_csv_with_lone_cr_keeps_lines() {
        let input = "\"\r\"zz,1\n\r,.";
        // csv クレートは単独の CR でレコードを区切るため、CSV として並べると `\r,.` が 2 行に分かれる
        assert_eq!(sort_lines(input, false), "\r,.\n\"\r\"zz,1");
    }

    /// 降順で空行が最後に来ても、空行の数を保つこと
    #[test]
    fn test_sort_lines_descending_keeps_trailing_blank_line() {
        let output = sort_lines("\na\nb", true);
        assert_eq!(output, "b\na\n\n");
        assert_eq!(output.lines().collect::<Vec<_>>(), ["b", "a", ""]);
        // 並べ替え済みの結果は再度並べ替えても変わらない
        assert_eq!(sort_lines(&output, true), output);
    }

    /// 重複行削除で末尾の空行を残すこと
    #[test]
    fn test_remove_duplicate_lines_keeps_trailing_blank_line() {
        assert_eq!(remove_duplicate_lines("a\n\na\n"), "a\n\n");
        assert!(matches!(remove_duplicate_lines("a\n\n"), Cow::Borrowed(_)));
    }

    /// 空行削除の基本動作できるようにすること
    #[test]
    fn test_remove_empty_lines() {
        let input = "line1\n\n  \nline2\n\t\nline3";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_empty_lines(input), expected);
    }

    /// 重複行削除(出現順序維持)できるようにすること
    #[test]
    fn test_remove_duplicate_lines() {
        let input = "line1\nline2\nline1\nline3\nline2";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_duplicate_lines(input), expected);
    }

    /// 空文字列は変更なしで返るようにすること
    #[test]
    fn test_sort_empty() {
        assert!(matches!(sort_lines("", false), Cow::Borrowed(_)));
        assert!(matches!(remove_empty_lines(""), Cow::Borrowed(_)));
        assert!(matches!(remove_duplicate_lines(""), Cow::Borrowed(_)));
    }

    /// 既にソート済みの場合は Borrowed を返す (変更なし)できるようにすること
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

    /// `is_likely_csv`: カンマ区切りで複数列かつ行数が一致すればCSV判定できるようにすること
    #[test]
    fn test_is_likely_csv_true() {
        // 2列 × 2行
        assert!(is_likely_csv("a,b\nc,d"));
        // 3列 × 2行
        assert!(is_likely_csv("1,2,3\n4,5,6"));
        // 2列 × 3行
        assert!(is_likely_csv("banana,2\napple,1\ncherry,3"));
    }

    /// `is_likely_csv`: 1列しかない場合はCSVでないできるようにすること
    #[test]
    fn test_is_likely_csv_false_single_column() {
        assert!(!is_likely_csv("apple\nbanana\ncherry"));
    }

    /// `is_likely_csv`: 1行だけではCSVと断定しないできるようにすること
    #[test]
    fn test_is_likely_csv_false_single_row() {
        assert!(!is_likely_csv("a,b,c"));
        assert!(!is_likely_csv("hello,world"));
    }

    /// `is_likely_csv`: 列数が揃っていない場合はCSVでないできるようにすること
    #[test]
    fn test_is_likely_csv_false_inconsistent_columns() {
        assert!(!is_likely_csv("a,b,c\nd,e"));
        assert!(!is_likely_csv("1,2\n3,4,5"));
    }

    /// CSVレコードの昇順ソートできるようにすること
    #[test]
    fn test_sort_csv_records_asc() {
        let input = "banana,2\napple,1\ncherry,3";
        let output = sort_lines(input, false);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "apple,1");
        assert_eq!(lines[1], "banana,2");
        assert_eq!(lines[2], "cherry,3");
    }

    /// CSVレコードの降順ソートできるようにすること
    #[test]
    fn test_sort_csv_records_desc() {
        let input = "banana,2\napple,1\ncherry,3";
        let output = sort_lines(input, true);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "cherry,3");
        assert_eq!(lines[1], "banana,2");
        assert_eq!(lines[2], "apple,1");
    }

    /// CSV 判定範囲より後ろに解析できない行があっても行を失わずに並び替えること
    ///
    /// `is_likely_csv` は先頭 7 レコードしか見ないため、9 行目の不正行は判定をすり抜ける
    #[test]
    fn test_sort_csv_keeps_unparsable_record_after_detection_window() {
        let mut rows: Vec<String> = (0..10).map(|i| format!("a{i},b")).collect();
        rows[8] = "x".to_string();
        let input = rows.join("\n");
        assert!(is_likely_csv(&input));

        // 全行が残り、行単位で並び替えた結果と一致する
        let output = sort_lines(&input, false);
        let mut sorted_input: Vec<&str> = input.lines().collect();
        sorted_input.sort_by_key(|l| l.to_lowercase());
        assert_eq!(output.lines().collect::<Vec<_>>(), sorted_input);
    }

    /// CRLF 入力で空行削除しても CRLF が保持されるようにすること
    #[test]
    fn test_remove_empty_lines_crlf() {
        let input = "line1\r\n\r\nline2\r\n";
        let output = remove_empty_lines(input);
        assert_eq!(output, "line1\r\nline2");
    }

    /// CRLF 入力で重複行削除しても CRLF が保持されるようにすること
    #[test]
    fn test_remove_duplicate_lines_crlf() {
        let input = "line1\r\nline2\r\nline1\r\n";
        let output = remove_duplicate_lines(input);
        assert_eq!(output, "line1\r\nline2");
    }
}

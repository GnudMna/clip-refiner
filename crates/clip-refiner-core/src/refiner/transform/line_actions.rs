use std::borrow::Cow;
use std::cmp::Ordering;

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
    sort_with(text, descending, false)
}

/// 行またはCSVレコード単位でテキストを自然順に並び替える
///
/// `sort_lines` と同じ単位・判定で並び替えるが、連続する数字を数値として比較する
/// (例: `2` は `10` より前になる)。比較時は大文字小文字を区別しない
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `descending` - 降順にする場合は `true`、昇順にする場合は `false`
///
/// # Returns
/// * `Cow<'_, str>` - 並び替え後のテキスト。変更がない場合は元の文字列への参照を返す。
pub fn sort_lines_natural(text: &str, descending: bool) -> Cow<'_, str> {
    sort_with(text, descending, true)
}

/// 比較方法を指定して行またはCSVレコード単位で並び替える
///
/// # Arguments
/// * `text` - 並び替える対象のテキスト
/// * `descending` - 降順にする場合は `true`
/// * `natural` - 連続する数字を数値として比較する場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - 並び替え後のテキスト。変更がない場合は元の文字列への参照を返す。
fn sort_with(text: &str, descending: bool, natural: bool) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let line_ending = super::utils::detect_line_ending(text);

    // クォート内改行を含むレコードが無い場合や、CSV 判定の範囲外に解析できないレコードがある場合は行単位で並び替える
    // csv クレートは単独の CR もレコードの区切りとみなし、行単位の区切り (LF / CRLF) とずれるため、
    // 単独の CR を含むテキストも行単位で並び替える
    if !has_lone_cr(text)
        && is_likely_csv(text)
        && let Some(sorted) = sort_csv_records(text, line_ending, descending, natural)
    {
        return sorted;
    }
    sort_plain_lines(text, line_ending, descending, natural)
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

    let result = super::utils::join_lines(&lines, line_ending, text.ends_with('\n'));
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

    let result = super::utils::join_lines(&lines, line_ending, text.ends_with('\n'));
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
/// * `natural` - 連続する数字を数値として比較する場合は `true`
///
/// # Returns
/// * `Option<Cow<'_, str>>` - レコード単位で並び替えられたCSVテキスト。
///   行単位で並び替えるべき場合は `None`
fn sort_csv_records<'a>(
    text: &'a str,
    line_ending: &str,
    descending: bool,
    natural: bool,
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

    let compare = key_comparator(natural);
    if descending {
        entries.sort_by(|a, b| compare(&b.0, &a.0));
    } else {
        entries.sort_by(|a, b| compare(&a.0, &b.0));
    }

    let lines: Vec<&str> = entries.iter().map(|(_, raw)| *raw).collect();
    let result = super::utils::join_lines(&lines, line_ending, text.ends_with('\n'));
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
/// * `natural` - 連続する数字を数値として比較する場合は `true`
///
/// # Returns
/// * `Cow<'_, str>` - 行単位で並び替えられたテキスト
fn sort_plain_lines<'a>(
    text: &'a str,
    line_ending: &str,
    descending: bool,
    natural: bool,
) -> Cow<'a, str> {
    // (比較キー, 元の行)
    let mut entries: Vec<(String, &str)> = text
        .lines()
        .map(|line| (line.to_lowercase(), line))
        .collect();
    let compare = key_comparator(natural);
    if descending {
        entries.sort_by(|a, b| compare(&b.0, &a.0));
    } else {
        entries.sort_by(|a, b| compare(&a.0, &b.0));
    }

    let lines: Vec<&str> = entries.iter().map(|(_, line)| *line).collect();
    let result = super::utils::join_lines(&lines, line_ending, text.ends_with('\n'));
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 比較キーの比較関数を選ぶ
///
/// # Arguments
/// * `natural` - 連続する数字を数値として比較する場合は `true`
///
/// # Returns
/// * `fn(&str, &str) -> Ordering` - 辞書順または自然順の比較関数
fn key_comparator(natural: bool) -> fn(&str, &str) -> Ordering {
    if natural {
        natural_cmp
    } else {
        |a, b| a.cmp(b)
    }
}

/// 連続する数字を数値として文字列を比較する
///
/// 数字以外は文字単位で比較し、数字の連続 (ASCII 数字のみ) は桁数に依らず数値の大小で比較する。
/// 先頭の 0 は無視するため、数値が等しい場合は次の部分の比較に進む。
/// 桁数の大きい数字も扱えるよう、数値へ変換せず桁数と文字列で比較する。
///
/// # Arguments
/// * `a` - 比較する文字列 (小文字化済み)
/// * `b` - 比較する文字列 (小文字化済み)
///
/// # Returns
/// * `Ordering` - `a` が `b` より前なら `Less`、後なら `Greater`、同順位なら `Equal`
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let (num_a, rest_a) = split_digits(a);
                let (num_b, rest_b) = split_digits(b);
                let (num_a, num_b) = (num_a.trim_start_matches('0'), num_b.trim_start_matches('0'));
                // 先頭の 0 を除いた桁数が多いほど大きい。同じ桁数なら文字列比較が数値比較に一致する
                let order = num_a.len().cmp(&num_b.len()).then_with(|| num_a.cmp(num_b));
                if order != Ordering::Equal {
                    return order;
                }
                (a, b) = (rest_a, rest_b);
            }
            (Some(ca), Some(cb)) => {
                if ca != cb {
                    return ca.cmp(&cb);
                }
                (a, b) = (&a[ca.len_utf8()..], &b[cb.len_utf8()..]);
            }
        }
    }
}

/// 文字列の先頭にある ASCII 数字の連続とその残りに分ける
///
/// # Arguments
/// * `s` - 分割する文字列
///
/// # Returns
/// * `(&str, &str)` - 先頭の数字の連続と、その後ろの文字列
fn split_digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
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
        assert_eq!(sort_lines(input, false), "\r\na,1\r\nz,\"x\r\ny\"\r\n");
        // 空行が最後に来る場合は、末尾の改行が行の終端とみなされて消えないよう改行を 1 つ足す
        assert_eq!(sort_lines(input, true), "z,\"x\r\ny\"\r\na,1\r\n\r\n");

        // 先頭・途中・末尾の空行を行単位の並び替えと同じ数だけ残し、元の末尾の改行も保つ
        let input = "\n\nz,\"a\nb\"\n\n\nc,d\n\n";
        assert_eq!(sort_lines(input, false), "\n\n\n\n\nc,d\nz,\"a\nb\"\n");
    }

    /// クォート内改行が無ければ、CSV とみなせても元の行の文字列のまま並び替えること
    #[test]
    fn test_sort_csv_like_lines_keeps_original_text() {
        let input = "log(\"y\", 2)\nlog(\"x\", 1)";
        assert!(is_likely_csv(input));
        assert_eq!(sort_lines(input, false), "log(\"x\", 1)\nlog(\"y\", 2)");
    }

    /// クォート内改行が無ければ、CSV とみなせても空行を残し、元の末尾の改行も保つこと
    ///
    /// 空行は空文字列として比較するため、昇順では先頭に来る
    #[test]
    fn test_sort_csv_like_lines_keeps_blank_lines() {
        assert_eq!(sort_lines("b,1\n\na,2\n", false), "\na,2\nb,1\n");
        assert_eq!(sort_lines("b,1\n\na,2", false), "\na,2\nb,1");
    }

    /// 行操作は元の末尾の改行の有無を保つこと
    #[test]
    fn test_line_actions_keep_trailing_newline() {
        // 末尾が改行 1 つの場合
        assert_eq!(remove_duplicate_lines("a\nb\na\n"), "a\nb\n");
        assert_eq!(sort_lines("b\na\n", false), "a\nb\n");
        assert_eq!(remove_empty_lines("a\n\nb\n"), "a\nb\n");
        // 末尾に改行が無い場合は付けない
        assert_eq!(remove_duplicate_lines("a\nb\na"), "a\nb");
        assert_eq!(sort_lines("b\na", false), "a\nb");
        assert_eq!(remove_empty_lines("a\n\nb"), "a\nb");
        // 変更が無ければ元の文字列を返す
        assert!(matches!(sort_lines("a\nb\n", false), Cow::Borrowed(_)));
        assert!(matches!(remove_empty_lines("a\nb\n"), Cow::Borrowed(_)));
        // 残る行が無ければ空文字列になる
        assert_eq!(remove_empty_lines("\n \n"), "");
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

    /// 自然順の昇順で数字を数値として並び替えられるようにすること
    #[test]
    fn test_sort_natural_asc() {
        assert_eq!(sort_lines_natural("10\n9\n2", false), "2\n9\n10");
        // 辞書順では 10 が先頭になる
        assert_eq!(sort_lines("10\n9\n2", false), "10\n2\n9");
        // 数字の前後に文字がある場合も、数字の連続ごとに比較する
        assert_eq!(
            sort_lines_natural("file10.txt\nfile2.txt\nFile1.txt", false),
            "File1.txt\nfile2.txt\nfile10.txt"
        );
    }

    /// 自然順の降順で数字を数値として並び替えられるようにすること
    #[test]
    fn test_sort_natural_desc() {
        assert_eq!(sort_lines_natural("2\n10\n9", true), "10\n9\n2");
    }

    /// 自然順で先頭の 0 を無視し、桁あふれする長さの数字も比較できるようにすること
    #[test]
    fn test_sort_natural_leading_zeros_and_long_digits() {
        assert_eq!(
            sort_lines_natural("a010\na9\na0002", false),
            "a0002\na9\na010"
        );
        assert_eq!(
            sort_lines_natural(
                "x100000000000000000000000000000\nx99999999999999999999999999999",
                false
            ),
            "x99999999999999999999999999999\nx100000000000000000000000000000"
        );
        // 数値が等しい行は元の順序を保つ
        assert_eq!(sort_lines_natural("a01\na1\na001", false), "a01\na1\na001");
    }

    /// 自然順で数字がない行や空行を含んでも、並び替え済みの結果が変わらないようにすること
    #[test]
    fn test_sort_natural_mixed_and_idempotent() {
        let output = sort_lines_natural("b\n\n10\na2\na10\n1", false);
        assert_eq!(output, "\n1\n10\na2\na10\nb");
        assert!(matches!(
            sort_lines_natural(&output, false),
            Cow::Borrowed(_)
        ));
        assert!(matches!(sort_lines_natural("", false), Cow::Borrowed(_)));
    }

    /// 自然順でもクォート内改行を含む CSV をレコード単位で並び替えられるようにすること
    #[test]
    fn test_sort_natural_csv_records() {
        let input = "10,\"x\ny\"\n2,b\n9,c";
        assert_eq!(sort_lines_natural(input, false), "2,b\n9,c\n10,\"x\ny\"");
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
        assert_eq!(output, "line1\r\nline2\r\n");
    }

    /// CRLF 入力で重複行削除しても CRLF が保持されるようにすること
    #[test]
    fn test_remove_duplicate_lines_crlf() {
        let input = "line1\r\nline2\r\nline1\r\n";
        let output = remove_duplicate_lines(input);
        assert_eq!(output, "line1\r\nline2\r\n");
    }
}

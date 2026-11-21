use std::borrow::Cow;
use std::cmp::Ordering;

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
/// RFC 4180 の CSV として読めるテキストはレコード単位、読めないテキストは行単位で並び替える。
/// どちらも単位の先頭行で比較するため、行単位の結果がたまたま CSV として読めても、
/// 再度並び替えたときに順序が変わらない (同順位は元の順序を保つ)
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

    let units = split_units(text);
    // 改行コードは位置ごとに元のまま残し、並び替えるのは行の内容だけにする
    let endings: Vec<&str> = units.iter().map(|(_, ending)| *ending).collect();

    // (比較キー, 元の文字列)
    let mut entries: Vec<(String, &str)> = units
        .into_iter()
        .map(|(unit, _)| (unit.lines().next().unwrap_or("").to_lowercase(), unit))
        .collect();
    let compare = key_comparator(natural);
    if descending {
        entries.sort_by(|a, b| compare(&b.0, &a.0));
    } else {
        entries.sort_by(|a, b| compare(&a.0, &b.0));
    }

    let sorted: Vec<(&str, &str)> = entries
        .iter()
        .zip(endings)
        .map(|((_, unit), ending)| (*unit, ending))
        .collect();
    let result = super::utils::join_lines(&sorted, text.ends_with('\n'));
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

// ======================================================================
// 行削除
// ======================================================================
/// テキストから空行を削除する
///
/// 改行のみの行、または空白文字(スペース、タブなど)のみの行を取り除く
/// 残る行の改行コードは行ごとに元のまま維持される (CRLF と LF が混在していても変えない)
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

    let lines: Vec<(&str, &str)> = super::utils::split_line_endings(text)
        .into_iter()
        .filter(|(line, _)| !line.trim().is_empty())
        .collect();

    let result = super::utils::join_lines(&lines, text.ends_with('\n'));
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// 重複行またはCSVレコードを削除する(出現順序を維持する)
///
/// `sort_lines` と同じ単位で重複を判定し、最初に出現した単位のみを残して他を削除する。
/// クォート内に改行を含む CSV のレコードは、改行を含めて 1 つの単位として比較する
/// セット(HashSet)を使用して効率的に重複を判定する
///
/// # Arguments
/// * `text` - 処理対象のテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 重複が削除されたテキスト。変更がない場合は元の文字列への参照を返す。
pub fn remove_duplicate_lines(text: &str) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let mut seen = std::collections::HashSet::new();
    let mut units = Vec::new();

    for (unit, ending) in split_units(text) {
        if seen.insert(unit) {
            units.push((unit, ending));
        }
    }

    let result = super::utils::join_lines(&units, text.ends_with('\n'));
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

// ======================================================================
// CSV のレコード分割
// ======================================================================
/// テキストを並び替え・重複判定の単位に分ける
///
/// RFC 4180 の CSV として読めるテキストはレコード単位、読めないテキストは行単位で分ける
///
/// # Arguments
/// * `text` - 分割する対象のテキスト
///
/// # Returns
/// * `Vec<(&str, &str)>` - (改行を除いた各単位の元の文字列, 単位末尾の改行コード) の組。
///   最終行が改行で終わっていなければ、その改行コードは空文字列
fn split_units(text: &str) -> Vec<(&str, &str)> {
    split_csv_records(text).unwrap_or_else(|| super::utils::split_line_endings(text))
}

/// フィールドの読み取り状態
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldState {
    /// フィールドの先頭
    Start,
    /// クォートで囲まない値の途中
    Unquoted,
    /// クォートで囲んだ値の途中
    Quoted,
    /// クォートで囲んだ値の中でクォートを読んだ直後 (閉じクォートかエスケープ)
    QuoteInQuoted,
}

/// テキストを RFC 4180 の CSV としてレコード単位に分ける
///
/// クォートで囲んだフィールドだけが改行・カンマ・`""` (エスケープしたクォート) を含められる。
/// 閉じクォートの直後はカンマか行末に限り、空行を除くレコードのフィールド数は 2 以上でそろっている必要がある。
/// 改行は LF と CRLF で、単独の CR は行単位の並び替え (`str::lines`) と同じく改行とみなさない。
/// 空行は空のレコードとして残す
///
/// # Arguments
/// * `text` - 分割する対象のテキスト
///
/// # Returns
/// * `Option<Vec<(&str, &str)>>` - (改行を除いた各レコードの元の文字列, レコード末尾の改行コード) の組。
///   最終レコードが改行で終わっていなければ、その改行コードは空文字列。CSV として読めない場合は `None`
fn split_csv_records(text: &str) -> Option<Vec<(&str, &str)>> {
    let bytes = text.as_bytes();
    let mut records = Vec::new();
    let mut field_count: Option<usize> = None;
    let mut fields = 1;
    let mut state = FieldState::Start;
    let mut record_start = 0;

    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        // クォートの外の改行 (LF / CRLF) の長さ
        let line_ending_len = match byte {
            _ if state == FieldState::Quoted => 0,
            b'\n' => 1,
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
            _ => 0,
        };
        if line_ending_len > 0 {
            let record = &text[record_start..i];
            if !record.is_empty() && *field_count.get_or_insert(fields) != fields {
                return None;
            }
            records.push((record, &text[i..i + line_ending_len]));
            i += line_ending_len;
            record_start = i;
            fields = 1;
            state = FieldState::Start;
            continue;
        }

        state = match (state, byte) {
            (FieldState::Start | FieldState::Unquoted | FieldState::QuoteInQuoted, b',') => {
                fields += 1;
                FieldState::Start
            }
            (FieldState::Quoted, b'"') => FieldState::QuoteInQuoted,
            // `""` はエスケープしたクォートとして値に含める
            (FieldState::Start | FieldState::QuoteInQuoted, b'"') | (FieldState::Quoted, _) => {
                FieldState::Quoted
            }
            // クォートで囲まない値の途中にクォートは置けず、閉じクォートの直後はカンマか行末に限る
            (FieldState::Unquoted, b'"') | (FieldState::QuoteInQuoted, _) => return None,
            (FieldState::Start | FieldState::Unquoted, _) => FieldState::Unquoted,
        };
        i += 1;
    }

    // 閉じていないクォートは CSV として読めない
    if state == FieldState::Quoted {
        return None;
    }
    if record_start < text.len() {
        if *field_count.get_or_insert(fields) != fields {
            return None;
        }
        records.push((&text[record_start..], ""));
    }
    // 1 列だけのテキストは、引用符で囲んだ複数行の文章と見分けられないため CSV として読まない
    if field_count.is_some_and(|count| count < 2) {
        return None;
    }
    Some(records)
}

// ======================================================================
// ソートユーティリティ
// ======================================================================
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

    /// クォート内改行が無ければ、元の行の文字列のまま並び替えること
    #[test]
    fn test_sort_csv_like_lines_keeps_original_text() {
        let input = "log(\"y\", 2)\nlog(\"x\", 1)";
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

    /// クォート内改行を含む CSV の重複削除をレコード単位で行うこと
    ///
    /// 行単位で比べると、別のレコードの断片 (`x"` など) が重複と判定されて壊れる
    #[test]
    fn test_remove_duplicate_lines_csv_records() {
        // 同じレコードは改行を含めて 1 つにまとめる
        let input = "a,\"x\ny\"\nb,1\na,\"x\ny\"\n";
        assert_eq!(remove_duplicate_lines(input), "a,\"x\ny\"\nb,1\n");
        // 後半の行が同じでも、レコードが異なれば両方残す
        let input = "a,\"p\nq\"\nb,\"p\nq\"\n";
        assert_eq!(remove_duplicate_lines(input), input);
        assert!(matches!(remove_duplicate_lines(input), Cow::Borrowed(_)));
        // CRLF を保ち、クォート内の改行は書き換えない
        let input = "a,\"x\r\ny\"\r\nb,1\r\na,\"x\r\ny\"\r\n";
        assert_eq!(remove_duplicate_lines(input), "a,\"x\r\ny\"\r\nb,1\r\n");
        // CSV として読めないテキストは行単位のまま扱う
        assert_eq!(remove_duplicate_lines("a,\"x\nb\na,\"x"), "a,\"x\nb");
    }

    /// 空行削除の基本動作できるようにすること
    #[test]
    fn test_remove_empty_lines() {
        let input = "line1\n\n  \nline2\n\t\nline3";
        let expected = "line1\nline2\nline3";
        assert_eq!(remove_empty_lines(input), expected);
    }

    /// CRLF と LF が混在していても、行ごとの改行コードを保持すること
    #[test]
    fn test_line_actions_keep_mixed_line_endings() {
        // 空行削除は残る行の改行コードをそのまま使う
        assert_eq!(remove_empty_lines("a\r\n\r\nb\n\nc\r\n"), "a\r\nb\nc\r\n");
        // 重複行削除も同様
        assert_eq!(
            remove_duplicate_lines("a\r\nb\na\r\nc\r\n"),
            "a\r\nb\nc\r\n"
        );
        // 並び替えは改行コードの並びを位置ごとに保つ
        assert_eq!(sort_lines("b\r\na\nc\r\n", false), "a\r\nb\nc\r\n");
        assert_eq!(sort_lines("b\r\na\nc", true), "c\r\nb\na");
        // CSV のレコード単位でも同様
        assert_eq!(
            sort_lines("b,1\r\na,\"x\ny\"\nc,3", false),
            "a,\"x\ny\"\r\nb,1\nc,3"
        );
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

    /// `split_csv_records`: クォート内の改行・カンマ・エスケープしたクォートをレコードに含めること
    #[test]
    fn test_split_csv_records_quoted_fields() {
        assert_eq!(
            split_csv_records("a,\"x\ny\"\nb,\"p,\"\"q\"\"\""),
            Some(vec![("a,\"x\ny\"", "\n"), ("b,\"p,\"\"q\"\"\"", "")])
        );
        // CRLF はレコードの区切りから除き、クォート内の CRLF はそのまま残す
        assert_eq!(
            split_csv_records("\"x\r\ny\",1\r\n\r\na,2\r\n"),
            Some(vec![
                ("\"x\r\ny\",1", "\r\n"),
                ("", "\r\n"),
                ("a,2", "\r\n")
            ])
        );
    }

    /// `split_csv_records`: 1 レコードの CSV も読み、1 列のテキストは CSV として読まないこと
    #[test]
    fn test_split_csv_records_single_record_or_column() {
        assert_eq!(
            split_csv_records("\"x\na,x\n0\","),
            Some(vec![("\"x\na,x\n0\",", "")])
        );
        // 引用符で囲んだ複数行の文章と見分けられない
        assert_eq!(split_csv_records("\"a\nb\"\nc"), None);
        assert_eq!(split_csv_records("\"\n\n\"\nx"), None);
    }

    /// `split_csv_records`: RFC 4180 に従わないテキストは CSV として読まないこと
    #[test]
    fn test_split_csv_records_rejects_invalid_csv() {
        // クォートで囲まない値の途中のクォート
        assert_eq!(split_csv_records("log(\"x\", 1)"), None);
        // 閉じクォートの直後にカンマ・行末以外の文字
        assert_eq!(split_csv_records("\"a\"b,1"), None);
        // 閉じていないクォート
        assert_eq!(split_csv_records("\"a\nb,1"), None);
        // フィールド数の不一致 (空行は数えない)
        assert_eq!(split_csv_records("a,b,c\nd,e"), None);
        assert_eq!(
            split_csv_records("a,b\n\nc,d"),
            Some(vec![("a,b", "\n"), ("", "\n"), ("c,d", "")])
        );
    }

    /// 1 レコードだけの CSV は、クォート内の改行で行に分けず変更しないこと
    #[test]
    fn test_sort_single_csv_record_keeps_quoted_newlines() {
        let input = "\"x\na,x\n0\",";
        assert!(matches!(sort_lines(input, false), Cow::Borrowed(_)));
        assert!(matches!(sort_lines_natural(input, true), Cow::Borrowed(_)));
    }

    /// 行単位の結果が CSV として読めても、再度並び替えたときに順序が変わらないこと
    ///
    /// 閉じていないクォートを含むため行単位で並び替え、結果の `"x` と `0",` はクォート内改行を含むレコードになる
    #[test]
    fn test_sort_lines_result_readable_as_csv_is_stable() {
        let output = sort_lines("a,x\n0\",\n\"x", false);
        assert_eq!(output, "\"x\n0\",\na,x");
        assert_eq!(sort_lines(&output, false), output);
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

    /// 後ろの方にフィールド数の合わない行があっても行を失わずに並び替えること
    #[test]
    fn test_sort_csv_keeps_unparsable_record_after_many_rows() {
        let mut rows: Vec<String> = (0..10).map(|i| format!("a{i},\"b\nc\"")).collect();
        rows[8] = "x".to_string();
        let input = rows.join("\n");
        assert_eq!(split_csv_records(&input), None);

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

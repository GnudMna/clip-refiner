use std::borrow::Cow;
use std::fmt::Write;

use pulldown_cmark::{Options, Parser, html};

use crate::security::is_within_parser_limit;

// ======================================================================
// Markdown → HTML
// ======================================================================
/// `Markdown` を `HTML` へ変換する
///
/// 入力されたMarkdownテキストを解析し、HTML形式の文字列に変換する
/// テーブル、脚注、取り消し線、タスクリスト、スマートパンクチュエーションなどの拡張機能をサポートしている
///
/// # Arguments
/// * `text` - 変換対象のMarkdownテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のHTML文字列。変更がない場合は元の文字列への参照を返す。
pub fn markdown_to_html(text: &str) -> Cow<'_, str> {
    if !is_within_parser_limit(text) {
        tracing::debug!("Markdown 入力が上限を超えているためスキップ (markdown_to_html)");
        return Cow::Borrowed(text);
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    let result = html_output.trim().to_string();

    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// `Markdown` をリッチテキスト貼り付け向けの `HTML` へ変換する
///
/// Word / Outlook は罫線指定のない表を枠なしで表示するため、表に罫線と余白を付ける
///
/// # Arguments
/// * `text` - 変換対象のMarkdownテキスト
///
/// # Returns
/// * `String` - `CF_HTML` の断片として書き込む HTML
pub(crate) fn markdown_to_rich_text_html(text: &str) -> String {
    markdown_to_html(text).replace(
        "<table>",
        r#"<table border="1" cellpadding="4" style="border-collapse: collapse;">"#,
    )
}

// ======================================================================
// Excel → Markdown 変換
// ======================================================================
/// Excel(TSV)形式のテキストをMarkdownの表形式へ変換する
///
/// タブ区切り(TSV)のテキストを解析し、Markdownのテーブル形式に変換する
/// セル内の改行は `<br>` タグに置換され、パイプ記号(`|`)はエスケープされる
/// 1行目はヘッダーとして扱う
///
/// # Arguments
/// * `text` - 変換対象のTSVテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のMarkdownテーブル文字列。パースに失敗した場合は元の文字列を返す。
pub fn excel_to_markdown_table(text: &str) -> Cow<'_, str> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let records: Vec<csv::StringRecord> = reader
        .records()
        .filter_map(|r| match r {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::debug!(
                    "TSV レコードのパースに失敗 (excel_to_markdown_table): {}",
                    e
                );
                None
            }
        })
        .collect();
    if records.is_empty() {
        return Cow::Borrowed(text);
    }

    let max_cols = records
        .iter()
        .map(csv::StringRecord::len)
        .max()
        .unwrap_or(0);
    if max_cols == 0 {
        return Cow::Borrowed(text);
    }

    let mut markdown = String::new();

    // 1行目をヘッダー行として出力
    let header_row = &records[0];
    markdown.push('|');
    for i in 0..max_cols {
        let cell = header_row.get(i).unwrap_or("").trim();
        let processed = cell
            .replace("\r\n", "<br>")
            .replace(['\r', '\n'], "<br>")
            .replace('|', "\\|");
        let _ = write!(markdown, " {processed} |");
    }
    markdown.push('\n');

    // 区切り行(`|---|`)を出力
    markdown.push('|');
    for _ in 0..max_cols {
        markdown.push_str("---|");
    }
    markdown.push('\n');

    // 2行目以降をデータ行として出力
    for record in records.iter().skip(1) {
        markdown.push('|');
        for i in 0..max_cols {
            let cell = record.get(i).unwrap_or("").trim();
            let processed = cell
                .replace("\r\n", "<br>")
                .replace(['\r', '\n'], "<br>")
                .replace('|', "\\|");
            let _ = write!(markdown, " {processed} |");
        }
        markdown.push('\n');
    }

    let result = markdown.trim().to_string();
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

// ======================================================================
// Markdown → Excel 変換
// ======================================================================
/// Markdownの表形式テキストをExcel(TSV)形式へ変換する
///
/// パイプ区切りのMarkdownテーブルを解析し、タブ区切り(TSV)に変換する
/// セル内の `<br>` (`<br/>`・`<br />` を含む) は改行に戻し、エスケープされたパイプ (`\|`) は復元する
/// インラインコード内の `|` はセルの区切りとみなさない
/// 区切り行 (`|---|`) はスキップする
///
/// # Arguments
/// * `text` - 変換対象のMarkdownテーブル文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のTSV文字列。表が検出できない場合は元の文字列を返す
pub fn markdown_table_to_excel(text: &str) -> Cow<'_, str> {
    let records = parse_markdown_table_rows(text);
    if records.is_empty() {
        return Cow::Borrowed(text);
    }

    let max_cols = records.iter().map(Vec::len).max().unwrap_or(0);
    if max_cols == 0 {
        return Cow::Borrowed(text);
    }

    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_writer(Vec::new());

    for record in records {
        let row: Vec<&str> = (0..max_cols)
            .map(|i| record.get(i).map_or("", String::as_str))
            .collect();
        if let Err(e) = writer.write_record(&row) {
            tracing::debug!(
                "TSV レコードの書き込みに失敗 (markdown_table_to_excel): {}",
                e
            );
            return Cow::Borrowed(text);
        }
    }

    let bytes = match writer.into_inner() {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::debug!(
                "TSV ライターのフラッシュに失敗 (markdown_table_to_excel): {}",
                e
            );
            return Cow::Borrowed(text);
        }
    };

    let result = String::from_utf8(bytes).unwrap_or_default();
    let result = result.trim_end_matches('\n').to_string();

    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result)
    }
}

/// Markdownテーブル行をパースしてセル配列のリストを返す
///
/// 区切り行とみなすのは表の 2 行目 (見出し行の直後) だけとする (GFM と同じ)。
/// 見出しやデータ行のセルがすべて `-` でも (`| - | - |`) データとして残す
fn parse_markdown_table_rows(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    // 連続する表の行のうち何行目か (`|` を含まない行で表が途切れる)
    let mut row_in_table = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            row_in_table = 0;
            continue;
        }

        let cells = split_table_row(trimmed);
        row_in_table += 1;
        if row_in_table == 2 && is_separator_row(&cells) {
            continue;
        }

        records.push(cells.iter().map(|cell| unescape_table_cell(cell)).collect());
    }

    records
}

/// パイプ区切りのテーブル行をセルに分割する
///
/// `\|` はエスケープとして `|` に戻す。インラインコード (`` `a|b` ``) 内の `|` は区切りとみなさない
fn split_table_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);

    let chars: Vec<char> = line.chars().collect();
    let mut cells = Vec::new();
    let mut current = String::new();
    // 開いているインラインコードのバッククォート数 (0 はインラインコード外)
    let mut code_ticks = 0;
    let mut i = 0;

    while let Some(&ch) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        if ch == '\\' && next == Some('|') {
            current.push('|');
            i += 2;
        } else if ch == '\\' && next == Some('`') && code_ticks == 0 {
            // エスケープしたバッククォートはインラインコードを開かない
            current.push_str("\\`");
            i += 2;
        } else if ch == '`' {
            let run = backtick_run_len(&chars[i..]);
            if code_ticks == 0 {
                // 閉じる側が無いバッククォートは通常の文字として扱う
                if has_closing_backticks(&chars[i + run..], run) {
                    code_ticks = run;
                }
            } else if run == code_ticks {
                code_ticks = 0;
            }
            current.extend(&chars[i..i + run]);
            i += run;
        } else if ch == '|' && code_ticks == 0 {
            cells.push(std::mem::take(&mut current));
            i += 1;
        } else {
            current.push(ch);
            i += 1;
        }
    }
    cells.push(current);

    cells
}

/// 先頭から連続するバッククォートの数を返す
fn backtick_run_len(chars: &[char]) -> usize {
    chars.iter().take_while(|&&ch| ch == '`').count()
}

/// 同じ数のバッククォートの連なりが後続にあるかどうかを判定する
fn has_closing_backticks(chars: &[char], run: usize) -> bool {
    let mut i = 0;
    while i < chars.len() {
        let len = backtick_run_len(&chars[i..]);
        if len == run {
            return true;
        }
        i += len.max(1);
    }
    false
}

/// 区切り行 (`|---|`, `|:---:|` など) かどうかを判定する
///
/// 全セルが `-` を含む必要がある。空セルだけの行 (`|  |  |`) はデータ行として残す。
fn is_separator_row(cells: &[String]) -> bool {
    cells.iter().all(|cell| {
        let trimmed = cell.trim();
        trimmed.contains('-')
            && trimmed
                .chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
    })
}

/// セル内の `<br>` を改行に戻す (`\|` は `split_table_row` で復元済み)
///
/// `<br/>`・`<br />`・`<BR>` のような表記揺れも改行とみなす
fn unescape_table_cell(cell: &str) -> String {
    let cell = cell.trim();
    let mut result = String::with_capacity(cell.len());
    let mut rest = cell;

    while let Some(pos) = rest.find('<') {
        result.push_str(&rest[..pos]);
        let tag = &rest[pos..];
        if let Some(len) = br_tag_len(tag) {
            result.push('\n');
            rest = &tag[len..];
        } else {
            result.push('<');
            rest = &tag[1..];
        }
    }
    result.push_str(rest);

    result
}

/// 先頭が `<br>` タグ (`<br/>`・`<br />` を含み、大文字小文字を区別しない) ならその長さを返す
fn br_tag_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if !bytes
        .get(1..3)
        .is_some_and(|name| name.eq_ignore_ascii_case(b"br"))
    {
        return None;
    }
    let mut i = 3;
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if bytes.get(i) == Some(&b'/') {
        i += 1;
    }
    (bytes.get(i) == Some(&b'>')).then_some(i + 1)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 見出しと太字の基本的な Markdown→HTML 変換できるようにすること
    #[test]
    fn test_markdown_to_html_basic() {
        let input = "# Header\n**bold**";
        let output = markdown_to_html(input);
        assert!(output.contains("<h1>Header</h1>"));
    }

    /// 太字・斜体の変換できるようにすること
    #[test]
    fn test_markdown_to_html_inline() {
        let output = markdown_to_html("**bold** and *italic*");
        assert!(output.contains("<strong>bold</strong>"));
        assert!(output.contains("<em>italic</em>"));
    }

    /// `取り消し線の変換(ENABLE_STRIKETHROUGH)`できるようにすること
    #[test]
    fn test_markdown_to_html_strikethrough() {
        let output = markdown_to_html("~~strike~~");
        assert!(output.contains("<del>strike</del>"));
    }

    /// `テーブルの変換(ENABLE_TABLES)`できるようにすること
    #[test]
    fn test_markdown_to_html_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = markdown_to_html(input);
        assert!(output.contains("<table>"));
        assert!(output.contains("<th>"));
        assert!(output.contains("<td>"));
    }

    /// リッチテキスト向け HTML では表に罫線を付けるようにすること
    #[test]
    fn test_markdown_to_rich_text_html_adds_table_border() {
        let output = markdown_to_rich_text_html(
            "| A |
|---|
| 1 |",
        );
        assert!(
            output.starts_with(r#"<table border="1" cellpadding="4""#),
            "{output}"
        );
        assert!(!output.contains("<table>"));
    }

    /// `excel_to_markdown_table`: 基本的な TSV 変換できるようにすること
    #[test]
    fn test_excel_to_markdown_table_basic() {
        let input = "Name\tAge\nAlice\t30\nBob\t25";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("| Name | Age |"));
        assert!(output.contains("|---|---|"));
        assert!(output.contains("| Alice | 30 |"));
        assert!(output.contains("| Bob | 25 |"));
    }

    /// `excel_to_markdown_table`: セル内の `|` がエスケープされるようにすること
    #[test]
    fn test_excel_to_markdown_table_pipe_escape() {
        let input = "A|B\tC";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("A\\|B"));
    }

    /// `excel_to_markdown_table`: 空入力は元の文字列を返すようにすること
    #[test]
    fn test_excel_to_markdown_table_empty() {
        let input = "";
        assert_eq!(excel_to_markdown_table(input), input);
    }

    /// `excel_to_markdown_table`: ヘッダーのみ (データ行なし) も動作すること
    #[test]
    fn test_excel_to_markdown_table_header_only() {
        let input = "Col1\tCol2\tCol3";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("| Col1 | Col2 | Col3 |"));
        assert!(output.contains("|---|---|---|"));
    }

    /// `markdown_table_to_excel`: 基本的な Markdown 表変換できるようにすること
    #[test]
    fn test_markdown_table_to_excel_basic() {
        let input = "| Name | Age |\n|---|---|\n| Alice | 30 |\n| Bob | 25 |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "Name\tAge\nAlice\t30\nBob\t25");
    }

    /// `markdown_table_to_excel`: セル内の `\|` が復元されるようにすること
    #[test]
    fn test_markdown_table_to_excel_pipe_unescape() {
        let input = "| A\\|B | C |\n|---|---|\n| 1 | 2 |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "A|B\tC\n1\t2");
    }

    /// `markdown_table_to_excel`: セル内の `<br>` が改行に戻るようにすること
    #[test]
    fn test_markdown_table_to_excel_br_to_newline() {
        let input = "| Col |\n|---|\n| a<br>b |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "Col\n\"a\nb\"");
    }

    /// `markdown_table_to_excel`: `<br/>`・`<br />`・大文字の `<BR>` も改行に戻すこと
    #[test]
    fn test_markdown_table_to_excel_br_variants_to_newline() {
        let input = "| Col |\n|---|\n| a<br/>b<br />c<BR>d<Br  />e |";
        assert_eq!(markdown_table_to_excel(input), "Col\n\"a\nb\nc\nd\ne\"");

        // `<br` で始まる別のタグや閉じていないタグはそのまま残す
        let input = "| Col |\n|---|\n| <bra> <br <b>x |";
        assert_eq!(markdown_table_to_excel(input), "Col\n<bra> <br <b>x");
    }

    /// `markdown_table_to_excel`: インラインコード内の `|` をセルの区切りとみなさないこと
    #[test]
    fn test_markdown_table_to_excel_keeps_pipe_in_code_span() {
        let input = "| Cmd | Note |\n|---|---|\n| `a | b` | x |\n| ``c ` | d`` | y |";
        assert_eq!(
            markdown_table_to_excel(input),
            "Cmd\tNote\n`a | b`\tx\n``c ` | d``\ty"
        );

        // 閉じていないバッククォートやエスケープしたバッククォートはインラインコードを開かない
        let input = "| A | B |\n|---|---|\n| `x | y |\n| \\`p | q` |";
        assert_eq!(markdown_table_to_excel(input), "A\tB\n`x\ty\n\\`p\tq`");
    }

    /// `markdown_table_to_excel`: インラインコード内の `\|` も `|` に戻すこと
    #[test]
    fn test_markdown_table_to_excel_unescapes_pipe_in_code_span() {
        let input = "| Cmd |\n|---|\n| `a \\| b` |";
        assert_eq!(markdown_table_to_excel(input), "Cmd\n`a | b`");
    }

    /// `markdown_table_to_excel`: 中身が空の行を区切り行とみなさず残すこと
    #[test]
    fn test_markdown_table_to_excel_keeps_empty_row() {
        let input = "| A | B |\n|:---|---:|\n| 1 | 2 |\n|  |  |\n| 3 | 4 |";
        let output = markdown_table_to_excel(input);
        // 空行は空セルのみの TSV 行として位置を保つ
        assert_eq!(output, "A\tB\n1\t2\n\t\n3\t4");
    }

    /// `markdown_table_to_excel`: セルがすべて `-` の見出し・データ行を区切り行とみなさず残すこと
    #[test]
    fn test_markdown_table_to_excel_keeps_dash_only_rows() {
        let input = "| - | - |\n|---|---|\n| 1 | 2 |\n| - | :-: |";
        assert_eq!(markdown_table_to_excel(input), "-\t-\n1\t2\n-\t:-:");
    }

    /// `markdown_table_to_excel`: 表ごとに 2 行目の区切り行を読み飛ばすこと
    #[test]
    fn test_markdown_table_to_excel_skips_separator_per_table() {
        let input = "| A |\n|---|\n| 1 |\n\n| B |\n|---|\n| 2 |";
        assert_eq!(markdown_table_to_excel(input), "A\n1\nB\n2");
    }

    /// `markdown_table_to_excel`: 空入力は元の文字列を返すようにすること
    #[test]
    fn test_markdown_table_to_excel_empty() {
        let input = "";
        assert_eq!(markdown_table_to_excel(input), input);
    }

    /// `markdown_table_to_excel`: 表でないテキストは元の文字列を返すようにすること
    #[test]
    fn test_markdown_table_to_excel_non_table() {
        let input = "plain text";
        assert_eq!(markdown_table_to_excel(input), input);
    }

    /// `markdown_table_to_excel`: Excel→Markdown の往復変換が成立すること
    #[test]
    fn test_markdown_excel_roundtrip() {
        let tsv = "Name\tAge\nAlice\t30\nBob\t25";
        let markdown = excel_to_markdown_table(tsv);
        let back = markdown_table_to_excel(&markdown);
        assert_eq!(back, tsv);
    }
}

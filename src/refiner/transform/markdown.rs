use std::borrow::Cow;
use std::fmt::Write;

use crate::security::is_within_parser_limit;

use pulldown_cmark::{Options, Parser, html};

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
        crate::log_debug!("Markdown 入力が上限を超えているためスキップ (markdown_to_html)");
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
                crate::log_debug!(
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
/// セル内の `<br>` は改行に戻し、エスケープされたパイプ (`\|`) は復元する
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
            crate::log_debug!(
                "TSV レコードの書き込みに失敗 (markdown_table_to_excel): {}",
                e
            );
            return Cow::Borrowed(text);
        }
    }

    let bytes = match writer.into_inner() {
        Ok(bytes) => bytes,
        Err(e) => {
            crate::log_debug!(
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
fn parse_markdown_table_rows(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            continue;
        }

        let cells = split_table_row(trimmed);
        if cells.is_empty() || is_separator_row(&cells) {
            continue;
        }

        records.push(cells.iter().map(|cell| unescape_table_cell(cell)).collect());
    }

    records
}

/// パイプ区切りのテーブル行をセルに分割する (`\|` はエスケープとして扱う)
fn split_table_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            current.push('|');
            chars.next();
        } else if ch == '|' {
            cells.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    cells.push(current);

    cells
}

/// 区切り行 (`|---|`, `|:---:|` など) かどうかを判定する
fn is_separator_row(cells: &[String]) -> bool {
    cells.iter().all(|cell| {
        let trimmed = cell.trim();
        trimmed.is_empty()
            || trimmed
                .chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
    })
}

/// セル内のエスケープと `<br>` を復元する
fn unescape_table_cell(cell: &str) -> String {
    cell.trim().replace("\\|", "|").replace("<br>", "\n")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 見出しと太字の基本的な Markdown→HTML 変換
    #[test]
    fn test_markdown_to_html_basic() {
        let input = "# Header\n**bold**";
        let output = markdown_to_html(input);
        assert!(output.contains("<h1>Header</h1>"));
    }

    /// 太字・斜体の変換
    #[test]
    fn test_markdown_to_html_inline() {
        let output = markdown_to_html("**bold** and *italic*");
        assert!(output.contains("<strong>bold</strong>"));
        assert!(output.contains("<em>italic</em>"));
    }

    /// `取り消し線の変換(ENABLE_STRIKETHROUGH)`
    #[test]
    fn test_markdown_to_html_strikethrough() {
        let output = markdown_to_html("~~strike~~");
        assert!(output.contains("<del>strike</del>"));
    }

    /// `テーブルの変換(ENABLE_TABLES)`
    #[test]
    fn test_markdown_to_html_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = markdown_to_html(input);
        assert!(output.contains("<table>"));
        assert!(output.contains("<th>"));
        assert!(output.contains("<td>"));
    }

    /// `excel_to_markdown_table`: 基本的な TSV 変換
    #[test]
    fn test_excel_to_markdown_table_basic() {
        let input = "Name\tAge\nAlice\t30\nBob\t25";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("| Name | Age |"));
        assert!(output.contains("|---|---|"));
        assert!(output.contains("| Alice | 30 |"));
        assert!(output.contains("| Bob | 25 |"));
    }

    /// `excel_to_markdown_table`: セル内の `|` がエスケープされること
    #[test]
    fn test_excel_to_markdown_table_pipe_escape() {
        let input = "A|B\tC";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("A\\|B"));
    }

    /// `excel_to_markdown_table`: 空入力は元の文字列を返すこと
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

    /// `markdown_table_to_excel`: 基本的な Markdown 表変換
    #[test]
    fn test_markdown_table_to_excel_basic() {
        let input = "| Name | Age |\n|---|---|\n| Alice | 30 |\n| Bob | 25 |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "Name\tAge\nAlice\t30\nBob\t25");
    }

    /// `markdown_table_to_excel`: セル内の `\|` が復元されること
    #[test]
    fn test_markdown_table_to_excel_pipe_unescape() {
        let input = "| A\\|B | C |\n|---|---|\n| 1 | 2 |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "A|B\tC\n1\t2");
    }

    /// `markdown_table_to_excel`: セル内の `<br>` が改行に戻ること
    #[test]
    fn test_markdown_table_to_excel_br_to_newline() {
        let input = "| Col |\n|---|\n| a<br>b |";
        let output = markdown_table_to_excel(input);
        assert_eq!(output, "Col\n\"a\nb\"");
    }

    /// `markdown_table_to_excel`: 空入力は元の文字列を返すこと
    #[test]
    fn test_markdown_table_to_excel_empty() {
        let input = "";
        assert_eq!(markdown_table_to_excel(input), input);
    }

    /// `markdown_table_to_excel`: 表でないテキストは元の文字列を返すこと
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

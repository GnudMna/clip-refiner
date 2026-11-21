use std::borrow::Cow;

use pulldown_cmark::{Options, Parser, html};

// ======================================================================
// Markdown → HTML
// ======================================================================
/// MarkdownをHTMLへ変換する
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

    let max_cols = records.iter().map(|r| r.len()).max().unwrap_or(0);
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
        markdown.push_str(&format!(" {} |", processed));
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
            markdown.push_str(&format!(" {} |", processed));
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

    /// 取り消し線の変換(ENABLE_STRIKETHROUGH)
    #[test]
    fn test_markdown_to_html_strikethrough() {
        let output = markdown_to_html("~~strike~~");
        assert!(output.contains("<del>strike</del>"));
    }

    /// テーブルの変換(ENABLE_TABLES)
    #[test]
    fn test_markdown_to_html_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = markdown_to_html(input);
        assert!(output.contains("<table>"));
        assert!(output.contains("<th>"));
        assert!(output.contains("<td>"));
    }

    /// excel_to_markdown_table: 基本的な TSV 変換
    #[test]
    fn test_excel_to_markdown_table_basic() {
        let input = "Name\tAge\nAlice\t30\nBob\t25";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("| Name | Age |"));
        assert!(output.contains("|---|---|"));
        assert!(output.contains("| Alice | 30 |"));
        assert!(output.contains("| Bob | 25 |"));
    }

    /// excel_to_markdown_table: セル内の `|` がエスケープされること
    #[test]
    fn test_excel_to_markdown_table_pipe_escape() {
        let input = "A|B\tC";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("A\\|B"));
    }

    /// excel_to_markdown_table: 空入力は元の文字列を返すこと
    #[test]
    fn test_excel_to_markdown_table_empty() {
        let input = "";
        assert_eq!(excel_to_markdown_table(input), input);
    }

    /// excel_to_markdown_table: ヘッダーのみ (データ行なし) も動作すること
    #[test]
    fn test_excel_to_markdown_table_header_only() {
        let input = "Col1\tCol2\tCol3";
        let output = excel_to_markdown_table(input);
        assert!(output.contains("| Col1 | Col2 | Col3 |"));
        assert!(output.contains("|---|---|---|"));
    }
}

use pulldown_cmark::{Options, Parser, html};

/// MarkdownをHTMLへ変換
///
/// # Arguments
/// * `text` - 変換するMarkdown文字列。
///
/// # Returns
/// * `String` - 変換されたHTML文字列。
pub fn markdown_to_html(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output.trim().to_string()
}

/// Excel(TSV)形式のテキストをMarkdownの表形式へ変換
///
/// # Arguments
/// * `text` - 変換するTSV文字列。
///
/// # Returns
/// * `String` - 変換されたMarkdown文字列。
pub fn excel_to_markdown_table(text: &str) -> String {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true) // 行ごとのカラム数が異なっても許容する
        .from_reader(text.as_bytes());

    let mut records: Vec<csv::StringRecord> = Vec::new();
    for result in reader.records() {
        if let Ok(record) = result {
            records.push(record);
        }
    }

    if records.is_empty() {
        return String::new();
    }

    // 最大のカラム数を計算
    let max_cols = records.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return String::new();
    }

    let mut markdown = String::new();

    // ヘッダー行の処理 (最初のレコード)
    let header_row = &records[0];
    markdown.push('|');
    for i in 0..max_cols {
        let cell = header_row.get(i).unwrap_or("").trim();
        // パイプ文字はエスケープし、改行コードは<br>に置換
        // replaceは文字列置換なので、\r\n -> <br><br> にならないように注意が必要だが
        // csv crate はデフォルトでレコード内の改行を保持する。
        // 単純な replace(['\r', '\n'], "<br>") だと \r\n は <br><br> になる可能性がある。
        // しかし、各プラットフォームでの挙動やブラウザの挙動を考えると <br> 1つが望ましい。
        // \r\n を先に <br> にして、残った \r や \n を <br> にするのが安全。
        let processed = cell
            .replace("\r\n", "<br>")
            .replace(['\r', '\n'], "<br>")
            .replace('|', "\\|");
        markdown.push_str(&format!(" {} |", processed));
    }
    markdown.push('\n');

    // セパレーター行 (|---|---|...)
    markdown.push('|');
    for _ in 0..max_cols {
        markdown.push_str("---|");
    }
    markdown.push('\n');

    // データ行 (2行目以降)
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

    markdown.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html_basic() {
        let input = "# Header\n**bold**";
        let output = markdown_to_html(input);
        assert_eq!(output, "<h1>Header</h1>\n<p><strong>bold</strong></p>");
    }

    #[test]
    fn test_markdown_to_html_table() {
        let input = "| a | b |\n|---|---|\n| 1 | 2 |";
        let output = markdown_to_html(input);
        assert!(output.contains("<table>"));
        assert!(output.contains("<td>1</td>"));
    }

    #[test]
    fn test_markdown_to_html_tasklist() {
        let input = "- [x] done\n- [ ] todo";
        let output = markdown_to_html(input);
        assert!(output.contains("checked"));
        assert!(output.contains("type=\"checkbox\""));
    }

    #[test]
    fn test_excel_to_markdown_table_basic() {
        let input = "Header1\tHeader2\nValue1\tValue2";
        let output = excel_to_markdown_table(input);
        let expected = "| Header1 | Header2 |\n|---|---|\n| Value1 | Value2 |";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_uneven() {
        let input = "H1\tH2\tH3\nV1\tV2"; // 2行目は2カラム
        let output = excel_to_markdown_table(input);
        let expected = "| H1 | H2 | H3 |\n|---|---|---|\n| V1 | V2 |  |";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_empty() {
        let input = "";
        let output = excel_to_markdown_table(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_excel_to_markdown_table_single_row() {
        let input = "Single\tRow";
        let output = excel_to_markdown_table(input);
        // セパレーター行は常に付く
        let expected = "| Single | Row |\n|---|---|";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_pipe_escape() {
        let input = "A|B\tC";
        let output = excel_to_markdown_table(input);
        let expected = "| A\\|B | C |\n|---|---|";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_multiline() {
        // "A" \t "B\nC"  (Excelではセル内改行を含むとダブルクォートで囲まれる)
        let input = "A\t\"B\nC\"";
        let output = excel_to_markdown_table(input);
        let expected = "| A | B<br>C |\n|---|---|";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_multiline_crlf() {
        let input = "A\t\"B\r\nC\"";
        let output = excel_to_markdown_table(input);
        let expected = "| A | B<br>C |\n|---|---|";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_markdown_to_html_complex() {
        let input = "> blockquote\n\n```rust\nfn main() {}\n```\n\n- list\n  - nested";
        let output = markdown_to_html(input);
        assert!(output.contains("<blockquote>"));
        assert!(output.contains("<pre><code class=\"language-rust\">"));
        assert!(output.contains("<ul>"));
        assert!(output.contains("<li>list"));
    }

    #[test]
    fn test_excel_to_markdown_table_empty_cells() {
        let input = "A\tB\tC\n1\t\t3";
        let output = excel_to_markdown_table(input);
        let expected = "| A | B | C |\n|---|---|---|\n| 1 |  | 3 |";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_excel_to_markdown_table_varying_columns() {
        // csv crate with flexible(true) fills missing columns with empty strings if the header is longer?
        // Or if rows are shorter? Let's verify behavior.
        // Actually, csv crate's records() returns whatever is in the line.
        // My implementation calculates max_cols.
        let input = "H1\tH2\nR1\nR2C1\tR2C2\tR2C3";
        let output = excel_to_markdown_table(input);
        // max_cols should be 3 (from last row)
        // Header needs padding
        // Row 1 needs padding
        let expected = "| H1 | H2 |  |\n|---|---|---|\n| R1 |  |  |\n| R2C1 | R2C2 | R2C3 |";
        assert_eq!(output, expected);
    }
}

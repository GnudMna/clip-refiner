use std::borrow::Cow;

use pulldown_cmark::{Options, Parser, html};

// ======================================================================
// Markdown → HTML
// ======================================================================
/// MarkdownをHTMLへ変換する
///
/// 入力されたMarkdownテキストを解析し、HTML形式の文字列に変換します。
/// テーブル、脚注、取り消し線、タスクリスト、スマートパンクチュエーションなどの拡張機能をサポートしています。
///
/// # Arguments
/// * `text` - 変換対象のMarkdownテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のHTML文字列。変更がない場合は元の文字列への参照を返します。
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
/// タブ区切り（TSV）のテキストを解析し、Markdownのテーブル形式に変換します。
/// セル内の改行は `<br>` タグに置換され、パイプ記号（`|`）はエスケープされます。
/// 1行目はヘッダーとして扱われます。
///
/// # Arguments
/// * `text` - 変換対象のTSVテキスト
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のMarkdownテーブル文字列。パースに失敗した場合は元の文字列を返します。
pub fn excel_to_markdown_table(text: &str) -> Cow<'_, str> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let records: Vec<csv::StringRecord> = reader.records().flatten().collect();
    if records.is_empty() {
        return Cow::Borrowed(text);
    }

    let max_cols = records.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return Cow::Borrowed(text);
    }

    let mut markdown = String::new();

    // Header row
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

    // Separator
    markdown.push('|');
    for _ in 0..max_cols {
        markdown.push_str("---|");
    }
    markdown.push('\n');

    // Data rows
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

    #[test]
    fn test_markdown_to_html_basic() {
        let input = "# Header\n**bold**";
        let output = markdown_to_html(input);
        assert!(output.contains("<h1>Header</h1>"));
    }
}

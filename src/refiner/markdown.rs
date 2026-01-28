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
}

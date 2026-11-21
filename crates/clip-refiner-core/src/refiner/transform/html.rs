use std::rc::Rc;

use crate::consts::MAX_HTML_NESTING_DEPTH;

use htmd::options::{BrStyle, BulletListMarker, HrStyle, Options};
use htmd::{HtmlToMarkdown, Node};

// ======================================================================
// 定数
// ======================================================================
/// Markdown へ変換せず本文ごと捨てる要素
///
/// `CF_HTML` の断片やページ全体のコピーに含まれる非表示要素を除く
const SKIPPED_TAGS: [&str; 8] = [
    "head", "link", "meta", "noscript", "script", "style", "template", "title",
];

// ======================================================================
// 変換結果
// ======================================================================
/// HTML→Markdown 変換の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HtmlToMarkdownResult {
    /// 変換した Markdown
    Converted(String),
    /// HTML タグを含まないため変換しなかった
    NotHtml,
    /// 要素の入れ子が `MAX_HTML_NESTING_DEPTH` を超えたため変換しなかった
    TooDeep,
}

// ======================================================================
// HTML → Markdown
// ======================================================================
/// `HTML` を `Markdown` へ変換する
///
/// 見出し・段落・強調・リンク・画像・リスト・引用・コードブロック・表 (GFM) に対応する
/// `script` / `style` などの非表示要素は出力しない
/// タグを含まないテキストは Markdown 記号のエスケープで壊さないよう変換しない
///
/// # Arguments
/// * `html` - 変換対象の HTML (断片またはページ全体)
///
/// # Returns
/// * `HtmlToMarkdownResult` - 変換結果、または変換しなかった理由
pub(crate) fn html_to_markdown(html: &str) -> HtmlToMarkdownResult {
    if !looks_like_html(html) {
        return HtmlToMarkdownResult::NotHtml;
    }

    let converter = HtmlToMarkdown::builder()
        .options(markdown_options())
        .skip_tags(SKIPPED_TAGS.to_vec())
        .build();

    let tree = match converter.html_to_tree(html) {
        Ok(tree) => tree,
        Err(e) => {
            tracing::debug!("HTML のパースに失敗 (html_to_markdown): {}", e);
            return HtmlToMarkdownResult::NotHtml;
        }
    };

    // 変換器の DOM 走査は再帰のため、走査前に深さを検査する
    if exceeds_nesting_depth(&tree, MAX_HTML_NESTING_DEPTH) {
        tracing::debug!("HTML の入れ子が上限を超えているためスキップ (html_to_markdown)");
        return HtmlToMarkdownResult::TooDeep;
    }

    let markdown = converter.tree_to_markdown(&tree);
    HtmlToMarkdownResult::Converted(markdown.trim().to_string())
}

/// HTML タグらしき `<名前` / `</名前` / `<!` を含むか判定する
///
/// `a < b` のような比較式だけのテキストは HTML とみなさない
fn looks_like_html(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(2).enumerate().any(|(i, pair)| {
        if pair[0] != b'<' {
            return false;
        }
        match pair[1] {
            b'!' => true,
            b'/' => bytes.get(i + 2).is_some_and(u8::is_ascii_alphabetic),
            next => next.is_ascii_alphabetic(),
        }
    })
}

/// 変換で使う Markdown の記法
///
/// 行末空白は保存先で消えやすいため、改行はバックスラッシュで表す
fn markdown_options() -> Options {
    Options {
        hr_style: HrStyle::Dashes,
        br_style: BrStyle::Backslash,
        bullet_list_marker: BulletListMarker::Dash,
        ul_bullet_spacing: 1,
        ol_number_spacing: 1,
        ..Options::default()
    }
}

/// DOM の深さが `limit` を超えるか判定する
///
/// スタック枯渇を避けるため、明示的なスタックで反復的に走査する
fn exceeds_nesting_depth(root: &Rc<Node>, limit: usize) -> bool {
    let mut stack = vec![(Rc::clone(root), 0_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > limit {
            return true;
        }
        for child in node.children.borrow().iter() {
            stack.push((Rc::clone(child), depth + 1));
        }
    }
    false
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    /// 変換しなかった場合は元の文字列を返す形で `html_to_markdown` を呼び出す
    fn html_to_markdown_text(html: &str) -> Cow<'_, str> {
        match html_to_markdown(html) {
            HtmlToMarkdownResult::Converted(markdown) if markdown != html => Cow::Owned(markdown),
            _ => Cow::Borrowed(html),
        }
    }

    /// 見出し・強調・リンクを Markdown へ変換できるようにすること
    #[test]
    fn converts_inline_and_heading() {
        let html = r#"<h2>Title</h2><p><strong>bold</strong> and <em>em</em> <a href="https://example.com/">link</a></p>"#;
        assert_eq!(
            html_to_markdown_text(html),
            "## Title\n\n**bold** and *em* [link](https://example.com/)"
        );
    }

    /// 箇条書きと番号付きリストを変換できるようにすること
    #[test]
    fn converts_lists() {
        let html = "<ul><li>a</li><li>b</li></ul><ol><li>one</li><li>two</li></ol>";
        assert_eq!(html_to_markdown_text(html), "- a\n- b\n\n1. one\n2. two");
    }

    /// 表を GFM の表へ変換できるようにすること
    #[test]
    fn converts_table() {
        let html = "<table><thead><tr><th>A</th><th>B</th></tr></thead>\
                    <tbody><tr><td>1</td><td>2</td></tr></tbody></table>";
        let markdown = html_to_markdown_text(html);
        assert!(markdown.contains("| A | B |"), "{markdown}");
        assert!(markdown.contains("| 1 | 2 |"), "{markdown}");
    }

    /// コードブロックをフェンス付きで変換できるようにすること
    #[test]
    fn converts_code_block() {
        let html = "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>";
        assert_eq!(html_to_markdown_text(html), "```rust\nfn main() {}\n```");
    }

    /// `script` / `style` / `head` の中身を出力しないようにすること
    #[test]
    fn skips_invisible_elements() {
        let html = "<html><head><title>t</title><style>p{color:red}</style></head>\
                    <body><script>alert(1)</script><p>visible</p></body></html>";
        assert_eq!(html_to_markdown_text(html), "visible");
    }

    /// ブラウザがコピーする `CF_HTML` 断片の装飾を除いて変換できるようにすること
    #[test]
    fn converts_browser_fragment() {
        let html = "<meta charset='utf-8'><span style=\"color: rgb(0, 0, 0);\">Hello </span>\
                    <b style=\"font-weight: 700;\">world</b>";
        assert_eq!(html_to_markdown_text(html), "Hello **world**");
    }

    /// タグを含まないテキストは変換しないようにすること
    #[test]
    fn leaves_plain_text() {
        for text in ["plain *text* with_underscore", "a < b && c > d", "1 <2"] {
            assert_eq!(html_to_markdown(text), HtmlToMarkdownResult::NotHtml);
            assert_eq!(html_to_markdown_text(text), text);
        }
    }

    /// 入れ子が上限を超える HTML は変換しないようにすること
    #[test]
    fn rejects_too_deep_nesting() {
        let depth = MAX_HTML_NESTING_DEPTH + 1;
        let html = format!("{}x{}", "<div>".repeat(depth), "</div>".repeat(depth));
        assert_eq!(html_to_markdown(&html), HtmlToMarkdownResult::TooDeep);
    }

    /// 上限付近の入れ子を Windows の既定スタック (1 MiB) 内で変換できるようにすること
    #[test]
    fn converts_nesting_near_limit_within_default_stack() {
        // 文書 → html → body → div 群 → b → テキストの深さが上限ちょうどになる
        let divs = MAX_HTML_NESTING_DEPTH - 4;
        let html = format!("{}<b>x</b>{}", "<div>".repeat(divs), "</div>".repeat(divs));
        let result = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(move || html_to_markdown(&html))
            .expect("スレッド生成に失敗")
            .join()
            .expect("変換スレッドが異常終了");
        assert_eq!(result, HtmlToMarkdownResult::Converted("**x**".to_string()));
    }
}

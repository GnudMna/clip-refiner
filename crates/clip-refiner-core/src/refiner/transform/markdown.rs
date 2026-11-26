use std::borrow::Cow;
use std::fmt::Write;

use pulldown_cmark::{Options, Parser, html};

use crate::consts::MAX_TABLE_CELLS;
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
/// * `Cow<'_, str>` - 変換後のHTML文字列。変更がない場合や入力がパーサの上限を超える場合は元の文字列への参照を返す
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
/// 表のセルを Markdown 用にエスケープする
///
/// 改行は `<br>` に置換する。文字としての `<br>` は `&lt;br>` にし、`&lt;br>` のような文字列は `&` を重ねる。`|` は `\|` にし、直前に連続するバックスラッシュは
/// GFM で `\\` が先に読まれるため重ねてエスケープする (`a\|b` → `a\\\|b`)
fn escape_table_cell(cell: &str) -> String {
    let mut result = String::with_capacity(cell.len());
    // 直前に連続するバックスラッシュの数
    let mut backslashes = 0;
    let mut chars = cell.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        match ch {
            // 文字としての `<br>` は改行に化けないよう `<` をエスケープする
            '<' if br_tag_len(&cell[idx..]).is_some() => {
                result.push_str("&lt;");
                backslashes = 0;
            }
            // `&lt;br>` などの文字列は、エスケープ済みと区別できるよう `&` を重ねる
            '&' if br_entity_amps(&cell[idx..]).is_some() => {
                result.push_str("&amp;");
                backslashes = 0;
            }
            '\r' => {
                // `\r\n` は 1 つの改行として扱う
                if chars.peek().map(|&(_, c)| c) == Some('\n') {
                    chars.next();
                }
                result.push_str("<br>");
                backslashes = 0;
            }
            '\n' => {
                result.push_str("<br>");
                backslashes = 0;
            }
            '|' => {
                result.extend(std::iter::repeat_n('\\', backslashes));
                result.push_str("\\|");
                backslashes = 0;
            }
            '\\' => {
                result.push('\\');
                backslashes += 1;
            }
            _ => {
                result.push(ch);
                backslashes = 0;
            }
        }
    }

    result
}

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
/// * `Cow<'_, str>` - 変換後のMarkdownテーブル文字列。読み取れるレコードが無い場合 (空入力など) は元の文字列を返す
pub fn excel_to_markdown_table(text: &str) -> Cow<'_, str> {
    let records = parse_excel_records(text);
    // 改行だけの入力は表にしない
    if records.iter().all(|r| r.len() == 1 && r[0].is_empty()) {
        return Cow::Borrowed(text);
    }

    let max_cols = records.iter().map(Vec::len).max().unwrap_or(0);
    if max_cols == 0 {
        return Cow::Borrowed(text);
    }
    if !is_within_table_cell_limit(records.len(), max_cols) {
        tracing::debug!("表のセル数が上限を超えているためスキップ (excel_to_markdown_table)");
        return Cow::Borrowed(text);
    }

    let mut markdown = String::new();

    // 1行目をヘッダー行として出力
    let header_row = &records[0];
    markdown.push('|');
    for i in 0..max_cols {
        let cell = header_row.get(i).map_or("", |c| c.trim());
        let processed = escape_table_cell(cell);
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
            let cell = record.get(i).map_or("", |c| c.trim());
            let processed = escape_table_cell(cell);
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

/// Excel のコピー形式 (TSV) をレコードとセルに分ける
///
/// Excel は改行・タブ・`"` を含むセルだけを `"` で囲み、中の `"` を `""` にする
/// 先頭が `"` でも、閉じ引用符の直後がタブ・改行・終端でなければ囲みとみなさず、`"` を含めた値として読む
/// 空行は 1 列の空セルのレコードとして残す (1 列の範囲の空セルで行がずれないようにする)
///
/// # Arguments
/// * `text` - 分割する対象の TSV テキスト
///
/// # Returns
/// * `Vec<Vec<String>>` - レコードごとのセルの一覧
fn parse_excel_records(text: &str) -> Vec<Vec<String>> {
    let bytes = text.as_bytes();
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        // フィールドを 1 つ読む
        let (field, next) = read_quoted_field(text, i).unwrap_or_else(|| {
            let end = bytes[i..]
                .iter()
                .position(|&b| matches!(b, b'\t' | b'\n' | b'\r'))
                .map_or(bytes.len(), |p| i + p);
            (text[i..end].to_string(), end)
        });
        row.push(field);
        i = next;

        // フィールドの後ろ: タブなら同じ行が続き、改行か終端ならレコードを閉じる
        match bytes.get(i) {
            Some(b'\t') => {
                i += 1;
                if i == bytes.len() {
                    row.push(String::new());
                }
                continue;
            }
            Some(b'\r') if bytes.get(i + 1) == Some(&b'\n') => i += 2,
            Some(_) => i += 1,
            None => {}
        }
        records.push(std::mem::take(&mut row));
    }
    if !row.is_empty() {
        records.push(row);
    }
    records
}

/// `start` から始まる `"` で囲んだセルを読む
///
/// # Arguments
/// * `text` - TSV テキスト全体
/// * `start` - セルの先頭のバイト位置
///
/// # Returns
/// * `Option<(String, usize)>` - (引用符を外した値, 閉じ引用符の次のバイト位置)
///   囲みとみなせない場合 (先頭が `"` でない・閉じていない・閉じ引用符の直後がタブ・改行・終端でない) は `None`
fn read_quoted_field(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut value = String::new();
    let mut seg_start = start + 1;
    let mut j = seg_start;
    while j < bytes.len() {
        if bytes[j] != b'"' {
            j += 1;
            continue;
        }
        value.push_str(&text[seg_start..j]);
        match bytes.get(j + 1) {
            // `""` はエスケープした引用符
            Some(b'"') => {
                value.push('"');
                j += 2;
                seg_start = j;
            }
            None | Some(b'\t' | b'\n' | b'\r') => return Some((value, j + 1)),
            Some(_) => return None,
        }
    }
    None
}

// ======================================================================
// Markdown → Excel 変換
// ======================================================================
/// Markdownの表形式テキストをExcel(TSV)形式へ変換する
///
/// 見出し行の直後に区切り行 (`|---|`) が続く塊だけを表とみなし、タブ区切り(TSV)に変換する (GFM と同じ)
/// 表は `|` を含まない行で終わる。表以外の行はそのまま残す
/// セル内の `<br>` (`<br/>`・`<br />` を含む) は改行に戻し、エスケープされたパイプ (`\|`) は復元する
/// インラインコード内の `|` も列の区切りとする (GFM と同じ)。コードフェンス内の行は表として読まない
///
/// # Arguments
/// * `text` - 変換対象のMarkdownテーブル文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後のTSV文字列。表が無い場合 (`|` を含むだけの文など) は元の文字列を返す
pub fn markdown_table_to_excel(text: &str) -> Cow<'_, str> {
    let lines: Vec<(&str, &str)> = text.split_inclusive('\n').map(split_line_ending).collect();
    let mut output = String::with_capacity(text.len());
    let mut found_table = false;
    let mut i = 0;
    // 開いているコードフェンス (フェンス文字, 長さ)。中の行は表とみなさない
    let mut fence: Option<(char, usize)> = None;

    while let Some(&(line, ending)) = lines.get(i) {
        if let Some(open) = fence {
            if is_closing_fence(line, open) {
                fence = None;
            }
            output.push_str(line);
            output.push_str(ending);
            i += 1;
            continue;
        }
        if let Some(open) = opening_fence(line) {
            fence = Some(open);
            output.push_str(line);
            output.push_str(ending);
            i += 1;
            continue;
        }

        let Some(header) = table_header(line, lines.get(i + 1).map(|&(next, _)| next)) else {
            output.push_str(line);
            output.push_str(ending);
            i += 1;
            continue;
        };

        // 見出し行と区切り行の後は `|` を含む行をデータ行として読む
        let mut rows = vec![header];
        let mut last_ending = lines.get(i + 1).map_or("", |&(_, e)| e);
        i += 2;
        while let Some(&(row, row_ending)) = lines
            .get(i)
            .filter(|(row, _)| row.contains('|') && opening_fence(row).is_none())
        {
            rows.push(split_table_row(row));
            last_ending = row_ending;
            i += 1;
        }

        // 行の区切りは見出し行の改行コード、表の直後の改行は最終行のものをそのまま残す
        let Some(tsv) = table_rows_to_tsv(&rows, ending) else {
            return Cow::Borrowed(text);
        };
        output.push_str(&tsv);
        output.push_str(last_ending);
        found_table = true;
    }

    if found_table {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(text)
    }
}

/// コードフェンスの開始行なら (フェンス文字, 長さ) を返す
///
/// 先頭に 3 つまでの空白を許し、`` ` `` または `~` が 3 つ以上続く行を開始とみなす。
/// `` ` `` のフェンスは info string に `` ` `` を含めない (GFM と同じ)
fn opening_fence(line: &str) -> Option<(char, usize)> {
    let (ch, len, rest) = fence_run(line)?;
    (ch != '`' || !rest.contains('`')).then_some((ch, len))
}

/// `open` で開いたコードフェンスを閉じる行か判定する
///
/// 同じフェンス文字が開始以上の長さで続き、後ろが空白だけの行を終了とみなす
fn is_closing_fence(line: &str, open: (char, usize)) -> bool {
    fence_run(line)
        .is_some_and(|(ch, len, rest)| ch == open.0 && len >= open.1 && rest.trim().is_empty())
}

/// 行頭 (3 つまでの空白の後) のフェンス文字の連なりを (文字, 長さ, 残り) に分ける
fn fence_run(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.bytes().take_while(|&b| b == b' ').count();
    if indent > 3 {
        return None;
    }
    let body = &line[indent..];
    let ch = body.chars().next().filter(|&c| c == '`' || c == '~')?;
    let len = body.chars().take_while(|&c| c == ch).count();
    (len >= 3).then(|| (ch, len, &body[len..]))
}

/// 1 行を本文と末尾の改行 (`\n` / `\r\n` / なし) に分ける
fn split_line_ending(line: &str) -> (&str, &str) {
    let Some(content) = line.strip_suffix('\n') else {
        return (line, "");
    };
    let content = content.strip_suffix('\r').unwrap_or(content);
    (content, &line[content.len()..])
}

/// 表の見出し行なら、そのセルを返す
///
/// 次の行が見出し行と同じ列数の区切り行である場合だけ表の見出しとみなす
fn table_header(line: &str, next: Option<&str>) -> Option<Vec<String>> {
    let next = next?;
    if !line.contains('|') || !next.contains('|') {
        return None;
    }
    let header = split_table_row(line);
    let separator = split_table_row(next);
    (header.len() == separator.len() && is_separator_row(&separator)).then_some(header)
}

/// 出力する表のセル数 (行数×列数) が上限以内か判定する
fn is_within_table_cell_limit(rows: usize, cols: usize) -> bool {
    rows.checked_mul(cols)
        .is_some_and(|cells| cells <= MAX_TABLE_CELLS)
}

/// 表の行 (区切り行を除く) を TSV に組み立てる
///
/// 列数は表の中で最も多い行に合わせ、足りないセルは空にする。行の区切りには `eol` を使い、末尾には付けない
/// 書き込みに失敗した場合・セル数が上限を超える場合は `None` を返す
fn table_rows_to_tsv(rows: &[Vec<String>], eol: &str) -> Option<String> {
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if !is_within_table_cell_limit(rows.len(), max_cols) {
        tracing::debug!("表のセル数が上限を超えているためスキップ (markdown_table_to_excel)");
        return None;
    }
    let mut records = Vec::with_capacity(rows.len());

    for row in rows {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_writer(Vec::new());
        let cells: Vec<String> = (0..max_cols)
            .map(|i| {
                row.get(i)
                    .map_or_else(String::new, |cell| unescape_table_cell(cell))
            })
            .collect();
        if let Err(e) = writer.write_record(&cells) {
            tracing::debug!(
                "TSV レコードの書き込みに失敗 (markdown_table_to_excel): {}",
                e
            );
            return None;
        }
        let bytes = match writer.into_inner() {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::debug!(
                    "TSV ライターのフラッシュに失敗 (markdown_table_to_excel): {}",
                    e
                );
                return None;
            }
        };
        let record = String::from_utf8(bytes).ok()?;
        records.push(record.strip_suffix('\n').unwrap_or(&record).to_string());
    }

    Some(records.join(eol))
}

/// パイプ区切りのテーブル行をセルに分割する
///
/// `\|` はエスケープとして `|` に戻す。GFM と同じく、インラインコード内の `|` も区切りとみなす
fn split_table_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);

    let chars: Vec<char> = line.chars().collect();
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut i = 0;

    while let Some(&ch) = chars.get(i) {
        if ch == '\\' && backslash_run_before_pipe(&chars[i..]).is_some() {
            // `|` 直前のバックスラッシュ列: 奇数個なら `|` はエスケープ済み、偶数個なら区切り
            let run = backslash_run_before_pipe(&chars[i..]).unwrap_or(0);
            current.extend(std::iter::repeat_n('\\', run / 2));
            if run % 2 == 1 {
                current.push('|');
                i += run + 1;
            } else {
                i += run;
            }
        } else if ch == '|' {
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

/// 先頭から連続するバックスラッシュの直後が `|` なら、その連続数を返す
fn backslash_run_before_pipe(chars: &[char]) -> Option<usize> {
    let run = chars.iter().take_while(|&&ch| ch == '\\').count();
    (chars.get(run) == Some(&'|')).then_some(run)
}

/// 区切り行 (`|---|`, `|:---:|` など) かどうかを判定する
///
/// 全セルが `-` を含む必要がある。空セルだけの行 (`|  |  |`) はデータ行として残す
fn is_separator_row(cells: &[String]) -> bool {
    cells.iter().all(|cell| {
        let trimmed = cell.trim();
        trimmed.contains('-')
            && trimmed
                .chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
    })
}

/// セル内の `<br>` を改行に戻し、文字としての `<br>` のエスケープを外す (`\|` は `split_table_row` で復元済み)
///
/// `<br/>`・`<br />`・`<BR>` のような表記揺れも改行とみなす。
/// `&lt;br>` は文字の `<br>` に、`&amp;lt;br>` は `&lt;br>` に戻す (`escape_table_cell` の逆)
fn unescape_table_cell(cell: &str) -> String {
    let cell = cell.trim();
    let mut result = String::with_capacity(cell.len());
    let mut rest = cell;

    while let Some(pos) = rest.find(['<', '&']) {
        result.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(len) = br_tag_len(tail) {
            result.push('\n');
            rest = &tail[len..];
        } else if let Some(amps) = br_entity_amps(tail) {
            if amps == 0 {
                // `&lt;` → `<`
                result.push('<');
                rest = &tail["&lt;".len()..];
            } else {
                // `&amp;` を 1 つ外す
                result.push('&');
                rest = &tail["&amp;".len()..];
            }
        } else {
            let ch_len = tail.chars().next().map_or(1, char::len_utf8);
            result.push_str(&tail[..ch_len]);
            rest = &tail[ch_len..];
        }
    }
    result.push_str(rest);

    result
}

/// `&` で始まる文字列が `&` + `amp;` の繰り返し + `lt;` + `br>` の形なら、`amp;` の繰り返し数を返す
///
/// 文字の `<br>` を `&lt;br>` とエスケープした表記と、その `&` を重ねたものを見分けるために使う
fn br_entity_amps(text: &str) -> Option<usize> {
    let mut rest = text.strip_prefix('&')?;
    let mut amps = 0;
    while let Some(next) = rest.strip_prefix("amp;") {
        rest = next;
        amps += 1;
    }
    br_name_len(rest.strip_prefix("lt;")?)?;
    Some(amps)
}

/// 先頭が `<br>` タグ (`<br/>`・`<br />` を含み、大文字小文字を区別しない) ならその長さを返す
fn br_tag_len(text: &str) -> Option<usize> {
    br_name_len(text.strip_prefix('<')?).map(|len| len + 1)
}

/// `<` の直後が `br>` (`br/>`・`br />` を含み、大文字小文字を区別しない) ならその長さを返す
fn br_name_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if !bytes
        .get(..2)
        .is_some_and(|name| name.eq_ignore_ascii_case(b"br"))
    {
        return None;
    }
    let mut i = 2;
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

    /// 取り消し線 (`ENABLE_STRIKETHROUGH`) を変換できるようにすること
    #[test]
    fn test_markdown_to_html_strikethrough() {
        let output = markdown_to_html("~~strike~~");
        assert!(output.contains("<del>strike</del>"));
    }

    /// テーブル (`ENABLE_TABLES`) を変換できるようにすること
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

    /// セル数が上限を超える表は加工せず元の文字列を返すこと
    #[test]
    fn test_table_conversion_cell_limit() {
        // 1 行だけ列が極端に多く、行数×列数が上限を超える入力は加工しないこと
        let wide = "\t".repeat(MAX_TABLE_CELLS / 2);
        let tsv = format!("{wide}\n{wide}\n{wide}\n");
        assert!(matches!(excel_to_markdown_table(&tsv), Cow::Borrowed(_)));

        let cells = "|".repeat(MAX_TABLE_CELLS / 2);
        let md = format!("{cells}\n{cells}\n|a|\n|b|\n|c|\n");
        assert!(matches!(markdown_table_to_excel(&md), Cow::Borrowed(_)));
    }

    /// 上限以内の表は変換すること
    #[test]
    fn test_table_conversion_within_cell_limit() {
        assert_eq!(
            excel_to_markdown_table("a\tb\n1\t2"),
            "| a | b |\n|---|---|\n| 1 | 2 |"
        );
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

    /// `excel_to_markdown_table`: `|` 直前のバックスラッシュも重ねてエスケープし、往復で元に戻ること
    #[test]
    fn test_excel_to_markdown_table_escapes_backslash_before_pipe() {
        let input = "a\\|b\tc\\\\|d\ne\\f\tg";
        let output = excel_to_markdown_table(input);
        // `\|` は `\\\|`、`\\|` は `\\\\\|` になり、`|` に続かない `\` はそのまま
        assert!(output.contains("a\\\\\\|b"));
        assert!(output.contains("c\\\\\\\\\\|d"));
        assert!(output.contains("e\\f"));
        // 往復で元のセル値に戻る
        assert_eq!(markdown_table_to_excel(&output), input);
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

    /// `excel_to_markdown_table`: 閉じ引用符の直後が区切りでないセルは引用符を残すこと
    #[test]
    fn test_excel_to_markdown_table_literal_quote() {
        assert_eq!(
            excel_to_markdown_table("山田\t\"至急\" 対応"),
            "| 山田 | \"至急\" 対応 |\n|---|---|"
        );
    }

    /// `excel_to_markdown_table`: 閉じていない引用符があっても表を 1 セルにまとめないこと
    #[test]
    fn test_excel_to_markdown_table_unclosed_quote() {
        assert_eq!(
            excel_to_markdown_table("\"abc\tdef\nghi\tjkl\nmno\tpqr"),
            "| \"abc | def |\n|---|---|\n| ghi | jkl |\n| mno | pqr |"
        );
    }

    /// `excel_to_markdown_table`: Excel が囲んだセルは改行を `<br>`、`""` を `"` にして読むこと
    #[test]
    fn test_excel_to_markdown_table_excel_quoted_cell() {
        assert_eq!(
            excel_to_markdown_table("a\t\"x\ny \"\"z\"\"\"\nb\tc"),
            "| a | x<br>y \"z\" |\n|---|---|\n| b | c |"
        );
    }

    /// `excel_to_markdown_table`: 1 列の範囲の空セルを空の行として残すこと
    #[test]
    fn test_excel_to_markdown_table_single_column_empty_cell() {
        assert_eq!(
            excel_to_markdown_table("見出し\r\nA\r\n\r\nB\r\n"),
            "| 見出し |\n|---|\n| A |\n|  |\n| B |"
        );
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

    /// `markdown_table_to_excel`: `<br/>`・`<br />`・大文字の `<BR>` も改行に戻すようにすること
    #[test]
    fn test_markdown_table_to_excel_br_variants_to_newline() {
        let input = "| Col |\n|---|\n| a<br/>b<br />c<BR>d<Br  />e |";
        assert_eq!(markdown_table_to_excel(input), "Col\n\"a\nb\nc\nd\ne\"");

        // `<br` で始まる別のタグや閉じていないタグはそのまま残す
        let input = "| Col |\n|---|\n| <bra> <br <b>x |";
        assert_eq!(markdown_table_to_excel(input), "Col\n<bra> <br <b>x");
    }

    /// `markdown_table_to_excel`: インラインコード内の `|` も GFM と同じく列の区切りとみなすこと
    #[test]
    fn test_markdown_table_to_excel_splits_pipe_in_code_span() {
        // バッククォートが片側だけのセルがあっても列数がずれない
        let input = "| A | B |\n|---|---|\n| `a | b` |";
        assert_eq!(markdown_table_to_excel(input), "A\tB\n`a\tb`");

        // 列数は最も多い行に合わせる
        let input = "| Cmd | Note |\n|---|---|\n| `a | b` | x |";
        assert_eq!(markdown_table_to_excel(input), "Cmd\tNote\t\n`a\tb`\tx");
    }

    /// `markdown_table_to_excel`: コードフェンス内の表を変換せずそのまま残すこと
    #[test]
    fn test_markdown_table_to_excel_skips_fenced_code() {
        let fenced = "```\n| a | b |\n|---|---|\n| 1 | 2 |\n```";
        assert_eq!(markdown_table_to_excel(fenced), fenced);

        // フェンスの前後の表は変換し、`~~~` や長いフェンスも扱う
        let input =
            "| x |\n|---|\n| 1 |\n~~~~\n| a |\n|---|\n~~~\n| b |\n|---|\n~~~~\n| y |\n|---|\n| 2 |";
        assert_eq!(
            markdown_table_to_excel(input),
            "x\n1\n~~~~\n| a |\n|---|\n~~~\n| b |\n|---|\n~~~~\ny\n2"
        );
    }

    /// 文字としての `<br>` が往復で改行に化けないこと
    #[test]
    fn test_markdown_excel_roundtrip_literal_br() {
        for cell in [
            "a<br>b",
            "<BR/>",
            "&lt;br&gt;",
            "&lt;br>",
            "&amp;lt;br>",
            "x &lt;br /> y",
            "<b>",
        ] {
            let tsv = format!("H\n{cell}");
            let markdown = excel_to_markdown_table(&tsv);
            assert_eq!(markdown_table_to_excel(&markdown), tsv, "{markdown}");
        }
        // 文字の `<br>` は Markdown 上で `<br>` タグにならない
        assert_eq!(
            excel_to_markdown_table("H\na<br>b"),
            "| H |\n|---|\n| a&lt;br>b |"
        );
    }

    /// `markdown_table_to_excel`: インラインコード内の `\|` も `|` に戻すようにすること
    #[test]
    fn test_markdown_table_to_excel_unescapes_pipe_in_code_span() {
        let input = "| Cmd |\n|---|\n| `a \\| b` |";
        assert_eq!(markdown_table_to_excel(input), "Cmd\n`a | b`");
    }

    /// `excel_to_markdown_table`: バッククォートと `|` を含むセルが往復で元に戻ること
    #[test]
    fn test_markdown_excel_roundtrip_backtick_pipe() {
        let tsv = "A\tB\n`a | b\tc` | d";
        let markdown = excel_to_markdown_table(tsv);
        assert_eq!(markdown_table_to_excel(&markdown), tsv);
    }

    /// `markdown_table_to_excel`: 中身が空の行を区切り行とみなさず残すようにすること
    #[test]
    fn test_markdown_table_to_excel_keeps_empty_row() {
        let input = "| A | B |\n|:---|---:|\n| 1 | 2 |\n|  |  |\n| 3 | 4 |";
        let output = markdown_table_to_excel(input);
        // 空行は空セルのみの TSV 行として位置を保つ
        assert_eq!(output, "A\tB\n1\t2\n\t\n3\t4");
    }

    /// `markdown_table_to_excel`: セルがすべて `-` の見出し・データ行を区切り行とみなさず残すようにすること
    #[test]
    fn test_markdown_table_to_excel_keeps_dash_only_rows() {
        let input = "| - | - |\n|---|---|\n| 1 | 2 |\n| - | :-: |";
        assert_eq!(markdown_table_to_excel(input), "-\t-\n1\t2\n-\t:-:");
    }

    /// `markdown_table_to_excel`: 表ごとに 2 行目の区切り行を読み飛ばすようにすること
    #[test]
    fn test_markdown_table_to_excel_skips_separator_per_table() {
        // 表の間の空行は残し、列数は表ごとにそろえる
        let input = "| A |\n|---|\n| 1 |\n\n| B | C |\n|---|---|\n| 2 |";
        assert_eq!(markdown_table_to_excel(input), "A\n1\n\nB\tC\n2\t");
    }

    /// `markdown_table_to_excel`: 表の前後にある表以外の行を消さずに残すこと
    #[test]
    fn test_markdown_table_to_excel_keeps_non_table_lines() {
        let input = "intro text\n| a | b |\n|---|---|\n| 1 | 2 |\noutro";
        assert_eq!(
            markdown_table_to_excel(input),
            "intro text\na\tb\n1\t2\noutro"
        );

        // 表以外の行は `"` やタブを含んでもそのまま残す
        let input = "say \"hi\"\tnow\n\n| a |\n|---|\n| 1 |";
        assert_eq!(markdown_table_to_excel(input), "say \"hi\"\tnow\n\na\n1");
    }

    /// `markdown_table_to_excel`: 区切り行の無い `|` を含む文は表とみなさず元の文字列を返すこと
    #[test]
    fn test_markdown_table_to_excel_ignores_pipe_without_separator() {
        // シェルのパイプ
        let input = "cat a | grep b";
        assert_eq!(markdown_table_to_excel(input), input);

        // 区切り行の無い表らしい行
        let input = "| a | b |\n| 1 | 2 |";
        assert_eq!(markdown_table_to_excel(input), input);

        // 区切り行の列数が見出し行と異なる
        let input = "| a | b |\n|---|\n| 1 | 2 |";
        assert_eq!(markdown_table_to_excel(input), input);

        // `|` を含まない区切り行 (見出しの下線や水平線)
        let input = "a | b\n---\nc";
        assert_eq!(markdown_table_to_excel(input), input);
    }

    /// `markdown_table_to_excel`: 改行コードと末尾の改行を保つこと
    #[test]
    fn test_markdown_table_to_excel_keeps_line_endings() {
        assert_eq!(
            markdown_table_to_excel("text\r\n\r\n| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\nafter\r\n"),
            "text\r\n\r\na\tb\r\n1\t2\r\nafter\r\n"
        );
        // 表で終わる入力の末尾の改行
        assert_eq!(markdown_table_to_excel("| a |\n|---|\n| 1 |\n"), "a\n1\n");
        // 混在した改行はそのまま残す
        assert_eq!(
            markdown_table_to_excel("x\r\n| a |\n|---|\n| 1 |\ny"),
            "x\r\na\n1\ny"
        );
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

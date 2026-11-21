/// 改行コードを判定する
///
/// # Arguments
/// * `text` - 判定対象のテキスト。
///
/// # Returns
/// * `&str` - 検出された改行コード（"\r\n" または "\n"）。
pub fn detect_line_ending(text: &str) -> &str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// 文字列を改行コードで分割し、各行に対して処理を行う共通ユーティリティ
///
/// # Arguments
/// * `text` - 処理対象の文字列
/// * `f` - 各行に対する処理。処理結果の文字列と、変更があったかどうかのフラグを返す
///
/// # Returns
/// * `Option<String>` - 少なくとも1行で変更があった場合は `Some(結合後のテキスト)` を返す
pub fn process_lines<F>(text: &str, f: F) -> Option<String>
where
    F: Fn(&str) -> Option<(String, bool)>,
{
    if text.is_empty() {
        return None;
    }

    let line_ending = detect_line_ending(text);
    let mut changed = false;

    let processed_lines: Vec<String> = text
        .split(line_ending)
        .map(|line| {
            if let Some((processed, line_changed)) = f(line) {
                if line_changed {
                    changed = true;
                }
                processed
            } else {
                line.to_string()
            }
        })
        .collect();

    if changed {
        Some(processed_lines.join(line_ending))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_line_ending() {
        assert_eq!(detect_line_ending("line1\nline2"), "\n");
        assert_eq!(detect_line_ending("line1\r\nline2"), "\r\n");
        assert_eq!(detect_line_ending("no newline"), "\n");
    }

    #[test]
    fn test_process_lines() {
        let input = "a\nb\nc";
        let result = process_lines(input, |line| {
            if line == "b" {
                Some(("B".to_string(), true))
            } else {
                None
            }
        });
        assert_eq!(result, Some("a\nB\nc".to_string()));

        let no_change = process_lines(input, |_| None);
        assert_eq!(no_change, None);
    }
}

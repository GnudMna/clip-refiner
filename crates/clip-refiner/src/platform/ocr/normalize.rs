// ======================================================================
// OCR テキスト正規化
// ======================================================================
/// OCR エンジンが日本語文字の間に挿入するスペースを除去する
pub(crate) fn normalize_ocr_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut normalized = String::with_capacity(text.len());

    for (index, ch) in chars.iter().enumerate() {
        if is_collapsible_ocr_space(*ch) {
            let prev = normalized.chars().last();
            let next = chars.get(index + 1).copied();
            if should_collapse_ocr_space(prev, next) {
                continue;
            }
        }
        normalized.push(*ch);
    }

    normalized
}

/// OCR 結果で除去候補となる空白文字かどうか
fn is_collapsible_ocr_space(ch: char) -> bool {
    ch == ' ' || ch == '\u{3000}'
}

/// 前後が日本語系文字ならスペースを詰める
fn should_collapse_ocr_space(prev: Option<char>, next: Option<char>) -> bool {
    matches!(
        (prev, next),
        (Some(left), Some(right)) if is_japanese_compact_char(left) && is_japanese_compact_char(right)
    )
}

/// スペースを詰めてよい日本語系文字かどうか
fn is_japanese_compact_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3001}'..='\u{303F}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9F}'
    )
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 漢字・かなの間に挿入されたスペースを除去する
    #[test]
    fn normalize_removes_spaces_between_japanese_chars() {
        assert_eq!(normalize_ocr_text("日 本 語 の テ ス ト"), "日本語のテスト");
    }

    /// 英単語間のスペースは維持する
    #[test]
    fn normalize_keeps_spaces_between_latin_words() {
        assert_eq!(normalize_ocr_text("hello world"), "hello world");
    }

    /// 英字と日本語の間のスペースは維持する
    #[test]
    fn normalize_keeps_space_between_latin_and_japanese() {
        assert_eq!(normalize_ocr_text("API の 説明"), "API の説明");
    }

    /// 改行は維持する
    #[test]
    fn normalize_keeps_line_breaks() {
        assert_eq!(normalize_ocr_text("あ い\nう え"), "あい\nうえ");
    }
}

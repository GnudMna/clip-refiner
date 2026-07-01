use crate::consts::{MAX_CLIPBOARD_TEXT_BYTES, MAX_PARSER_INPUT_BYTES};

// ======================================================================
// 入力サイズ制限
// ======================================================================
/// クリップボード本文が処理上限以内か判定する
///
/// バイト長で比較する (UTF-8 のマルチバイト文字も 1 バイトとして数える)
pub fn is_within_clipboard_limit(text: &str) -> bool {
    text.len() <= MAX_CLIPBOARD_TEXT_BYTES
}

/// パーサー入力が処理上限以内か判定する
pub fn is_within_parser_limit(text: &str) -> bool {
    text.len() <= MAX_PARSER_INPUT_BYTES
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 上限ちょうどは許可されること
    #[test]
    fn clipboard_limit_at_boundary() {
        let text = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES);
        assert!(is_within_clipboard_limit(&text));
    }

    /// 上限超過は拒否されること
    #[test]
    fn clipboard_limit_rejects_over_limit() {
        let text = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert!(!is_within_clipboard_limit(&text));
    }

    /// パーサー上限の境界値
    #[test]
    fn parser_limit_at_boundary() {
        let text = "a".repeat(MAX_PARSER_INPUT_BYTES);
        assert!(is_within_parser_limit(&text));
        let over = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        assert!(!is_within_parser_limit(&over));
    }
}

use crate::consts::{MAX_CLIPBOARD_TEXT_BYTES, MAX_PARSER_INPUT_BYTES};

// ======================================================================
// 入力サイズ制限
// ======================================================================
/// クリップボード本文が処理上限以内か判定する
///
/// バイト長で比較する (UTF-8 のマルチバイト文字は 1 文字を複数バイトとして数える)
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `bool` - 上限以内の場合は `true`
pub fn is_within_clipboard_limit(text: &str) -> bool {
    text.len() <= MAX_CLIPBOARD_TEXT_BYTES
}

/// パーサー入力が処理上限以内か判定する
///
/// # Arguments
/// * `text` - 判定対象のテキスト
///
/// # Returns
/// * `bool` - 上限以内の場合は `true`
pub fn is_within_parser_limit(text: &str) -> bool {
    text.len() <= MAX_PARSER_INPUT_BYTES
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 上限ちょうどは許可されるようにすること
    #[test]
    fn clipboard_limit_at_boundary() {
        let text = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES);
        assert!(is_within_clipboard_limit(&text));
    }

    /// 上限超過は拒否されるようにすること
    #[test]
    fn clipboard_limit_rejects_over_limit() {
        let text = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert!(!is_within_clipboard_limit(&text));
    }

    /// パーサー上限ちょうどは許可し、超過は拒否するようにすること
    #[test]
    fn parser_limit_at_boundary() {
        let text = "a".repeat(MAX_PARSER_INPUT_BYTES);
        assert!(is_within_parser_limit(&text));
        let over = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
        assert!(!is_within_parser_limit(&over));
    }
}

//! クリップボード変更シーケンス (`clipboard_win::raw::seq_num`)
//!
//! `OpenClipboard` を伴わない変更検知専用 API。本文 I/O の `OwnedClipboard` とは役割を分ける

// ======================================================================
// シーケンストークン
// ======================================================================
/// イベント監視が利用可能かどうか
///
/// `seq_num()` が取得できない場合は `false`
///
/// # Returns
/// * `bool` - クリップボードシーケンストークンを取得できる場合は `true`
pub fn supports_event_monitor() -> bool {
    token().is_some()
}

/// クリップボード変更を表すトークンを取得する
///
/// 本文を読み取らずに呼び出せる軽量な値。変更がない間は同じ値を返す
///
/// # Returns
/// * `Option<u64>` - 取得できた場合は `Some(トークン)`、未対応環境では `None`
pub fn token() -> Option<u64> {
    use clipboard_win::raw::seq_num;

    seq_num().map(|s| u64::from(s.get()))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// トークンを読み取れるようにすること
    #[test]
    fn clipboard_seq_reads_token() {
        let _ = token();
    }

    /// 変更がなければ連続呼び出しで同じトークンを返すようにすること
    #[test]
    fn consecutive_tokens_are_stable_when_unchanged() {
        if let (Some(first), Some(second)) = (token(), token()) {
            assert_eq!(first, second);
        }
    }

    /// イベント監視の可否とトークン取得可否が一致すること
    #[test]
    fn supports_event_monitor_matches_token_availability() {
        assert_eq!(supports_event_monitor(), token().is_some());
    }
}

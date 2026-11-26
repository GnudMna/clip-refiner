use crate::security::ContentFingerprint;
use crate::tray::state::ProcessedState;

// ======================================================================
// 加工判定
// ======================================================================
/// 加工を試みるべきか判定する。自身の書き戻しエコーをスキップする場合は `ps` を更新する
///
/// # Arguments
/// * `ps` - 前回の加工状態
/// * `text` - 現在のクリップボードテキスト
/// * `event_driven` - イベント駆動監視の場合は `true`、ポーリングの場合は `false`
///
/// # Returns
/// * `true` - 加工を試みる
/// * `false` - スキップする
pub(crate) fn should_process_clipboard(
    ps: &mut ProcessedState,
    text: &str,
    event_driven: bool,
) -> bool {
    if text.is_empty() {
        return false;
    }

    // 自身の書き戻しによるクリップボード変更イベントを1回無視
    if ps.matches_last_written(text) {
        ps.last_written = None;
        ps.last_seen = ContentFingerprint::from_text(text);
        return false;
    }

    // ポーリング: 前回と同じ内容なら加工しない
    event_driven || !ps.matches_last_seen(text)
}

/// 連続記録へ積むべきか判定する。自身の書き戻しエコーをスキップする場合は `ps` を更新する
///
/// 同一内容の再コピーも記録対象とする。自身の書き戻しエコーのみ 1 回スキップする
///
/// # Arguments
/// * `ps` - 前回の加工状態
/// * `text` - 現在のクリップボードテキスト
///
/// # Returns
/// * `true` - キューへ積む
/// * `false` - スキップする
pub(crate) fn should_record_continuous_clipboard(ps: &mut ProcessedState, text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    // キュー先頭の貼り付けや履歴復元など、自身の書き戻しエコーを 1 回無視
    if ps.matches_last_written(text) {
        ps.last_written = None;
        ps.last_seen = ContentFingerprint::from_text(text);
        return false;
    }

    true
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::ContentFingerprint;

    /// 空文字列は加工対象外とすること
    #[test]
    fn empty_text_is_not_processed() {
        let mut ps = ProcessedState::default();
        assert!(!should_process_clipboard(&mut ps, "", false));
        assert!(!should_process_clipboard(&mut ps, "", true));
    }

    /// ポーリング時は新しいテキストを加工対象とすること
    #[test]
    fn polling_processes_new_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("old"),
            last_written: None,
        };

        assert!(should_process_clipboard(&mut ps, "new", false));
    }

    /// 自身の書き戻しによる変更イベントを1回スキップすること
    #[test]
    fn should_skip_own_write_back_echo() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("input"),
            last_written: Some(ContentFingerprint::from_text("output")),
        };

        assert!(!should_process_clipboard(&mut ps, "output", true));
        assert_eq!(ps.last_written, None);
        assert!(ps.matches_last_seen("output"));
    }

    /// ポーリング時は同一テキストをスキップし、イベント時は再処理すること
    #[test]
    fn polling_skips_unchanged_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("same"),
            last_written: None,
        };

        assert!(!should_process_clipboard(&mut ps, "same", false));
        assert!(should_process_clipboard(&mut ps, "same", true));
    }

    /// イベント駆動時は加工済みテキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_processed_output() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("processed"),
            last_written: None,
        };

        // 加工結果と同じ文字列の再コピー(イベント駆動)も加工対象とする
        assert!(should_process_clipboard(&mut ps, "processed", true));
    }

    /// イベント駆動時は元テキストの再コピーも処理対象とすること
    #[test]
    fn event_mode_allows_recopy_of_source_text() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("processed"),
            last_written: None,
        };

        assert!(should_process_clipboard(&mut ps, "  source  ", true));
    }

    /// 連続記録では `last_seen` が同じでも再コピーを記録対象とすること
    #[test]
    fn continuous_record_allows_recopy_with_same_last_seen() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("same"),
            last_written: None,
        };

        assert!(should_record_continuous_clipboard(&mut ps, "same"));
    }

    /// 連続記録では自身の書き戻しエコーを 1 回スキップすること
    #[test]
    fn continuous_record_skips_own_write_back_echo() {
        let mut ps = ProcessedState {
            last_seen: ContentFingerprint::from_text("input"),
            last_written: Some(ContentFingerprint::from_text("output")),
        };

        assert!(!should_record_continuous_clipboard(&mut ps, "output"));
        assert_eq!(ps.last_written, None);
        assert!(ps.matches_last_seen("output"));
    }

    /// 連続記録では空白のみの文字列を無視すること
    #[test]
    fn continuous_record_ignores_whitespace_only() {
        let mut ps = ProcessedState::default();
        assert!(!should_record_continuous_clipboard(&mut ps, "   "));
    }
}

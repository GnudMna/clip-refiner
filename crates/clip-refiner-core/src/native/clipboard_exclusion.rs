//! パスワードマネージャー等が付与する「監視除外」クリップボードフォーマットの判定
//!
//! 本文を読む前に呼び、機密データとして除外指定されたコピーを監視対象から外す
//! 本文 I/O の `OwnedClipboard` (`arboard`) とは同時に `OpenClipboard` しない

// ======================================================================
// 定数
// ======================================================================
/// 存在するだけで監視・加工の対象外を示すフォーマット (Windows 10 以降の公式仕様)
const EXCLUDE_FROM_MONITOR_FORMAT: &str = "ExcludeClipboardContentFromMonitorProcessing";

/// 存在するだけでクリップボードビューアに無視を求める旧来の慣習フォーマット
const VIEWER_IGNORE_FORMAT: &str = "Clipboard Viewer Ignore";

/// DWORD 値 0 のとき Windows クリップボード履歴への保存を禁止するフォーマット
const CAN_INCLUDE_IN_HISTORY_FORMAT: &str = "CanIncludeInClipboardHistory";

// ======================================================================
// パブリック関数
// ======================================================================
/// 現在のクリップボード内容が監視除外を指定しているか判定する
///
/// 次のいずれかを満たす場合に除外とみなす
/// - `ExcludeClipboardContentFromMonitorProcessing` が存在する
/// - `Clipboard Viewer Ignore` が存在する
/// - `CanIncludeInClipboardHistory` の値が 0
///
/// 加工結果の書き戻しは元の除外フォーマットを引き継がず Windows 履歴やクラウド同期へ
/// 漏れるため、履歴禁止の指定も加工を含めて対象外とする
///
/// # Returns
/// * `bool` - 除外指定がある場合は `true`
pub(crate) fn is_excluded_from_monitoring() -> bool {
    use clipboard_win::{is_format_avail, register_format};

    let has_format = |name: &str| register_format(name).is_some_and(|f| is_format_avail(f.get()));

    if has_format(EXCLUDE_FROM_MONITOR_FORMAT) || has_format(VIEWER_IGNORE_FORMAT) {
        return true;
    }

    register_format(CAN_INCLUDE_IN_HISTORY_FORMAT)
        .filter(|f| is_format_avail(f.get()))
        .is_some_and(|f| history_disallowed(f.get()))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `CanIncludeInClipboardHistory` の DWORD 値が 0 か判定する
///
/// クリップボードを開けない・値を読めない場合は安全側に倒して除外とみなす
fn history_disallowed(format: u32) -> bool {
    use clipboard_win::{Clipboard, raw};

    // 短命 RAII。Drop で CloseClipboard する
    let Ok(_clipboard) = Clipboard::new_attempts(10) else {
        return true;
    };

    let mut data = Vec::new();
    if raw::get_vec(format, &mut data).is_err() {
        return true;
    }
    parse_history_flag(&data).is_none_or(|allowed| !allowed)
}

/// `CanIncludeInClipboardHistory` のバイト列を解釈する
///
/// # Arguments
/// * `data` - クリップボードから読んだ生データ (DWORD リトルエンディアン)
///
/// # Returns
/// * `Option<bool>` - 履歴への保存を許可する場合は `Some(true)`。4 バイト未満なら `None`
fn parse_history_flag(data: &[u8]) -> Option<bool> {
    let bytes: [u8; 4] = data.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes) != 0)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// DWORD 0 は履歴禁止として解釈すること
    #[test]
    fn parse_history_flag_zero_disallows() {
        assert_eq!(parse_history_flag(&[0, 0, 0, 0]), Some(false));
    }

    /// DWORD 1 は履歴許可として解釈すること
    #[test]
    fn parse_history_flag_one_allows() {
        assert_eq!(parse_history_flag(&[1, 0, 0, 0]), Some(true));
    }

    /// 4 バイト未満の不正データは解釈不能とすること
    #[test]
    fn parse_history_flag_rejects_short_data() {
        assert_eq!(parse_history_flag(&[]), None);
        assert_eq!(parse_history_flag(&[0, 0]), None);
    }

    /// 判定呼び出しがパニックしないようにすること
    #[test]
    fn is_excluded_from_monitoring_does_not_panic() {
        let _ = is_excluded_from_monitoring();
    }
}

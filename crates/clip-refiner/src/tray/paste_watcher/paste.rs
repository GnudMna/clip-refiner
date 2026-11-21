use super::super::state::AppState;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
use crate::refiner::TextClipboard;

// ======================================================================
// キュー先頭の貼り付け
// ======================================================================
/// 連続記録キュー先頭を取り出し、クリップボードへセットする
///
/// キューが空、または連続記録が無効な場合は何もしない
///
/// 本番の `Ctrl+V` 検知は [`try_paste_continuous_record_front_standalone`] を使う。
/// 統合テスト用ハーネスからのみ呼ぶ
///
/// # Returns
/// * `bool` - 先頭を取り出してクリップボードへ書き込めた場合は `true`
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn try_paste_continuous_record_front<C: TextClipboard>(
    state: &AppState,
    clipboard: &mut C,
) -> bool {
    if !state.with_config(|c| c.continuous_record_enabled) || state.continuous_record_len() == 0 {
        return false;
    }

    let Some(text) = state.discharge_continuous_record_front() else {
        return false;
    };

    if clipboard.set_text(text.to_string()).is_err() {
        state.enqueue_continuous_record(text.as_str());
        crate::log_error!("連続記録キュー先頭のクリップボード書き込みに失敗");
        return false;
    }

    state.record_clipboard_set(&text);
    true
}

/// 連続記録キュー先頭を取り出し、独立したクリップボード接続へ書き込む
///
/// `Ctrl+V` 検知時に貼り付けより先に同期実行するための経路
///
/// # Returns
/// * `bool` - 先頭を取り出してクリップボードへ書き込めた場合は `true`
pub(crate) fn try_paste_continuous_record_front_standalone(state: &AppState) -> bool {
    use arboard::Clipboard;

    if !state.with_config(|c| c.continuous_record_enabled) || state.continuous_record_len() == 0 {
        return false;
    }

    let Some(text) = state.discharge_continuous_record_front() else {
        return false;
    };

    let mut clipboard = match Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(e) => {
            state.enqueue_continuous_record(text.as_str());
            crate::log_error!("連続記録貼り付け用クリップボードの初期化に失敗: {:?}", e);
            return false;
        }
    };

    if clipboard.set_text(text.to_string()).is_err() {
        state.enqueue_continuous_record(text.as_str());
        crate::log_error!("連続記録キュー先頭のクリップボード書き込みに失敗");
        return false;
    }

    state.record_clipboard_set(&text);
    true
}

use std::mem;

use super::super::state::AppState;
use crate::refiner::TextClipboard;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT, VK_V,
};

// ======================================================================
// キュー先頭のクリップボード書き込み
// ======================================================================
/// 連続記録キュー先頭を取り出し、クリップボードへセットする
///
/// キューが空、または連続記録が無効な場合は何もしない
/// 書き込み失敗時はキューへ戻す
///
/// 本番のホットキー経路はワーカーの `PasteContinuousRecordFront` から呼ぶ
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `clipboard` - クリップボード書き込み先 (ワーカー所有インスタンス)
///
/// # Returns
/// * `bool` - 先頭を取り出してクリップボードへ書き込めた場合は `true`
pub(crate) fn try_paste_continuous_record_front<C: TextClipboard>(
    state: &AppState,
    clipboard: &mut C,
) -> bool {
    if !state.with_config(|c| c.continuous_record_enabled)
        || state.clipboard_session().continuous_record_len() == 0
    {
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

    state.clipboard_session().record_clipboard_set(&text);
    true
}

// ======================================================================
// Ctrl+V 送信
// ======================================================================
/// フォアグラウンドアプリへ `Ctrl+V` を送り、連続記録の貼り付けを完了する
///
/// クリップボード書き込みはワーカー側で済ませたうえで、メインスレッドから呼ぶ
pub(crate) fn send_continuous_record_ctrl_v() {
    if !simulate_ctrl_v() {
        crate::log_warn!("連続記録貼り付けのキー送信に失敗");
    }
}

/// 修飾キーを離してから `Ctrl+V` を送り、フォアグラウンドアプリへ貼り付けを通す
///
/// ホットキー (`Alt+Shift+X` 等) の修飾が押下中でも、合成 `Ctrl+V` が
/// 別の組み合わせにならないよう先に `Ctrl` / `Alt` / `Shift` を離す
fn simulate_ctrl_v() -> bool {
    let inputs = [
        keyboard_input(VK_CONTROL, true),
        keyboard_input(VK_MENU, true),
        keyboard_input(VK_SHIFT, true),
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let Ok(cbsize) = i32::try_from(mem::size_of::<INPUT>()) else {
        return false;
    };
    let Ok(expected) = u32::try_from(inputs.len()) else {
        return false;
    };
    // SAFETY: `inputs` はすべてキーボード入力として初期化済みで、`cbsize` は `INPUT` の実サイズ
    let sent = unsafe { SendInput(&inputs, cbsize) };
    sent == expected
}

fn keyboard_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::InMemoryTextClipboard;
    use crate::tray::state::test_app_state;

    /// 連続記録無効時は貼り付け排出を行わないようにすること
    #[test]
    fn paste_skips_when_continuous_record_disabled() {
        let state = test_app_state();
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        assert!(!try_paste_continuous_record_front(&state, &mut clipboard));
        assert_eq!(clipboard.text(), "keep");
    }

    /// キューが空のときは貼り付け排出を行わないようにすること
    #[test]
    fn paste_skips_when_queue_empty() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        assert!(!try_paste_continuous_record_front(&state, &mut clipboard));
        assert_eq!(clipboard.text(), "keep");
    }

    /// クリップボード書き込み失敗時はキューへ戻すようにすること
    #[test]
    fn paste_requeues_front_on_clipboard_write_failure() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        let mut clipboard = InMemoryTextClipboard::with_text("keep").fail_on_write();

        assert!(!try_paste_continuous_record_front(&state, &mut clipboard));
        assert_eq!(clipboard.text(), "keep");
        assert_eq!(state.clipboard_session().continuous_record_len(), 2);
        assert_eq!(
            state
                .clipboard_session()
                .get_continuous_record_entry(0)
                .map(|s| s.to_string()),
            Some("second".to_string())
        );
        assert_eq!(
            state
                .clipboard_session()
                .get_continuous_record_entry(1)
                .map(|s| s.to_string()),
            Some("first".to_string())
        );
    }

    /// 貼り付け排出成功時は先頭エントリを取り出すようにすること
    #[test]
    fn paste_discharges_front_entry() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        assert!(try_paste_continuous_record_front(&state, &mut clipboard));
        assert_eq!(clipboard.text(), "first");
        assert_eq!(state.clipboard_session().continuous_record_len(), 1);
        assert_eq!(
            state
                .clipboard_session()
                .get_continuous_record_entry(0)
                .map(|s| s.to_string()),
            Some("second".to_string())
        );
    }
}

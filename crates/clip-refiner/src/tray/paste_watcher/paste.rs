use std::mem;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
};

use super::super::state::AppState;
use crate::refiner::TextClipboard;

// ======================================================================
// キュー先頭のクリップボード書き込み
// ======================================================================
/// `try_paste_continuous_record_front` の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteFrontOutcome {
    /// キュー先頭をクリップボードへ書き込めた
    Written,
    /// キューが空、または連続記録が無効のため何もしなかった
    Skipped,
    /// クリップボードへの書き込みに失敗し、先頭をキューへ戻した
    WriteFailed,
}

impl PasteFrontOutcome {
    /// 貼り付け (`Ctrl+V`) を実行してよいか
    ///
    /// 書き込み失敗時は古いクリップボード内容が貼られるため実行しない
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - `Ctrl+V` を送ってよい場合は `true`
    pub(crate) fn should_send_paste(self) -> bool {
        !matches!(self, Self::WriteFailed)
    }
}

/// 連続記録キュー先頭を取り出し、クリップボードへセットする
///
/// キューが空、または連続記録が無効な場合は何もしない
/// 書き込み失敗時は取り出した先頭を元の位置 (先頭) へ戻す
///
/// 本番のホットキー経路はワーカーの `PasteContinuousRecordFront` から呼ぶ
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `clipboard` - クリップボード書き込み先 (ワーカー所有インスタンス)
///
/// # Returns
/// * `PasteFrontOutcome` - 書き込み成功・何もしなかった・書き込み失敗のいずれか
pub(crate) fn try_paste_continuous_record_front<C: TextClipboard>(
    state: &AppState,
    clipboard: &mut C,
) -> PasteFrontOutcome {
    if !state.with_config(|c| c.continuous_record_enabled)
        || state.clipboard_session().continuous_record_len() == 0
    {
        return PasteFrontOutcome::Skipped;
    }

    let Some(text) = state.discharge_continuous_record_front() else {
        return PasteFrontOutcome::Skipped;
    };

    if clipboard.set_text(text.to_string()).is_err() {
        state.restore_continuous_record_at(0, text.as_str());
        crate::log_error!("連続記録キュー先頭のクリップボード書き込みに失敗");
        return PasteFrontOutcome::WriteFailed;
    }

    state.clipboard_session().record_clipboard_set(&text);
    PasteFrontOutcome::Written
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
/// ホットキー (`Alt+Shift+X`・`Win+Shift+X` 等) の修飾が押下中でも、合成 `Ctrl+V` が
/// 別の組み合わせにならないよう先に `Ctrl` / `Alt` / `Shift` / `Win` を離す
fn simulate_ctrl_v() -> bool {
    let inputs = ctrl_v_inputs();
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

/// 押下中の修飾を離してから `Ctrl+V` を押下・解放する入力列
fn ctrl_v_inputs() -> [INPUT; 9] {
    [
        keyboard_input(VK_CONTROL, true),
        keyboard_input(VK_MENU, true),
        keyboard_input(VK_SHIFT, true),
        keyboard_input(VK_LWIN, true),
        keyboard_input(VK_RWIN, true),
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ]
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

    /// `Ctrl+V` の前に `Ctrl` / `Alt` / `Shift` / `Win` (左右) をすべて離すようにすること
    ///
    /// `Win` を離さないとフォアグラウンドアプリへ `Win+Ctrl+V` として届く
    #[test]
    fn ctrl_v_inputs_release_all_modifiers_before_paste() {
        // (仮想キー, 解放か) の列へ変換する
        let events: Vec<(VIRTUAL_KEY, bool)> = ctrl_v_inputs()
            .iter()
            .map(|input| {
                // SAFETY: `keyboard_input` は常に `ki` を初期化して返す
                let ki = unsafe { input.Anonymous.ki };
                (ki.wVk, ki.dwFlags == KEYEVENTF_KEYUP)
            })
            .collect();

        let v_down = events
            .iter()
            .position(|&event| event == (VK_V, false))
            .expect("V の押下が含まれる");
        for modifier in [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN] {
            let release = events
                .iter()
                .position(|&event| event == (modifier, true))
                .expect("修飾の解放が含まれる");
            assert!(release < v_down, "{modifier:?} の解放が V の押下より後");
        }
        // 末尾は押し直した Ctrl の解放
        assert_eq!(events.last(), Some(&(VK_CONTROL, true)));
    }

    /// 連続記録無効時は貼り付け排出を行わないようにすること
    #[test]
    fn paste_skips_when_continuous_record_disabled() {
        let state = test_app_state();
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        assert_eq!(
            try_paste_continuous_record_front(&state, &mut clipboard),
            PasteFrontOutcome::Skipped
        );
        assert_eq!(clipboard.text(), "keep");
    }

    /// キューが空のときは貼り付け排出を行わないようにすること
    #[test]
    fn paste_skips_when_queue_empty() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        let outcome = try_paste_continuous_record_front(&state, &mut clipboard);
        assert_eq!(outcome, PasteFrontOutcome::Skipped);
        // 空キュー時は現在のクリップボード内容をそのまま貼り付ける
        assert!(outcome.should_send_paste());
        assert_eq!(clipboard.text(), "keep");
    }

    /// クリップボード書き込み失敗時は先頭の位置のまま戻し、Ctrl+V を送らないようにすること
    #[test]
    fn paste_restores_front_on_clipboard_write_failure() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        state.enqueue_continuous_record("third");
        let mut clipboard = InMemoryTextClipboard::with_text("keep").fail_on_write();

        let outcome = try_paste_continuous_record_front(&state, &mut clipboard);
        assert_eq!(outcome, PasteFrontOutcome::WriteFailed);
        assert!(!outcome.should_send_paste());
        assert_eq!(clipboard.text(), "keep");
        // 並びは A,B,C のまま保たれる
        assert_eq!(state.clipboard_session().continuous_record_len(), 3);
        for (index, expected) in ["first", "second", "third"].into_iter().enumerate() {
            assert_eq!(
                state
                    .clipboard_session()
                    .get_continuous_record_entry(index)
                    .map(|s| s.to_string()),
                Some(expected.to_string())
            );
        }
    }

    /// 貼り付け排出成功時は先頭エントリを取り出すようにすること
    #[test]
    fn paste_discharges_front_entry() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.enqueue_continuous_record("first");
        state.enqueue_continuous_record("second");
        let mut clipboard = InMemoryTextClipboard::with_text("keep");

        let outcome = try_paste_continuous_record_front(&state, &mut clipboard);
        assert_eq!(outcome, PasteFrontOutcome::Written);
        assert!(outcome.should_send_paste());
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

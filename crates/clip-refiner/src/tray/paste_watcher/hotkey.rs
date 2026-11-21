use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::super::state::AppState;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, SendInput, UnregisterHotKey, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, PostThreadMessageW, TranslateMessage,
    WM_HOTKEY, WM_QUIT,
};

/// `RegisterHotKey` に渡すホットキー ID (スレッド内で一意であればよい)
const PASTE_HOTKEY_ID: i32 = 1;

/// キュー状態の再同期間隔
///
/// enqueue / discharge は別スレッドのため、登録状態を短周期で合わせる
const SYNC_POLL_INTERVAL: Duration = Duration::from_millis(50);

// ======================================================================
// 停止制御
// ======================================================================
/// 貼り付け監視スレッドの停止フラグと join ハンドル
pub struct PasteWatcherStop {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    thread_id: Arc<AtomicU32>,
}

impl Drop for PasteWatcherStop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let thread_id = self.thread_id.load(Ordering::Acquire);
        if thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ======================================================================
// スレッド起動
// ======================================================================
/// 連続記録キューがあるときだけ `Ctrl+V` を `RegisterHotKey` で捕捉する
///
/// 全キーを見る低レベルフックは使わない。キューが空、または連続記録 OFF のときは
/// ホットキーを解除し、通常の `Ctrl+V` を OS に任せる
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `PasteWatcherStop` - 監視停止制御用ハンドル
pub fn start(state: Arc<AppState>) -> PasteWatcherStop {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let thread_id = Arc::new(AtomicU32::new(0));
    let thread_id_for_loop = Arc::clone(&thread_id);
    let thread = thread::spawn(move || {
        run_hotkey_loop(&state, &stop_for_thread, &thread_id_for_loop);
    });
    PasteWatcherStop {
        stop,
        thread: Some(thread),
        thread_id,
    }
}

fn run_hotkey_loop(state: &AppState, stop: &AtomicBool, thread_id: &AtomicU32) {
    thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::Release);

    let mut arm = PasteHotkeyArmState::default();
    sync_paste_hotkey(state, &mut arm);

    let mut msg = MSG {
        hwnd: HWND::default(),
        message: 0,
        wParam: WPARAM(0),
        lParam: LPARAM(0),
        time: 0,
        pt: POINT { x: 0, y: 0 },
    };

    while !stop.load(Ordering::Relaxed) {
        let has_message = unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE) };
        if has_message.as_bool() {
            if msg.message == WM_QUIT {
                break;
            }
            if msg.message == WM_HOTKEY && msg.wParam.0 == PASTE_HOTKEY_ID as usize {
                handle_paste_hotkey(state, &mut arm);
                continue;
            }
            unsafe {
                let _ = TranslateMessage(&raw const msg);
                let _ = DispatchMessageW(&raw const msg);
            }
            continue;
        }

        sync_paste_hotkey(state, &mut arm);
        thread::sleep(SYNC_POLL_INTERVAL);
    }

    if arm.armed {
        unregister_paste_hotkey();
    }
}

// ======================================================================
// ホットキー登録
// ======================================================================
#[derive(Debug, Default)]
struct PasteHotkeyArmState {
    /// `RegisterHotKey` 済みかどうか
    armed: bool,
    /// 登録失敗を既にログしたか (連発抑制)
    fail_logged: bool,
}

/// `Ctrl+V` ホットキーを捕捉すべきか判定する
pub(crate) fn should_arm_paste_hotkey(state: &AppState) -> bool {
    state.with_config(|c| c.continuous_record_enabled)
        && state.clipboard_session().continuous_record_len() > 0
}

fn sync_paste_hotkey(state: &AppState, arm: &mut PasteHotkeyArmState) {
    let should_arm = should_arm_paste_hotkey(state);
    if should_arm == arm.armed {
        return;
    }
    if should_arm {
        if register_paste_hotkey() {
            arm.armed = true;
            arm.fail_logged = false;
        } else if !arm.fail_logged {
            crate::log_warn!("連続記録用 Ctrl+V ホットキーの登録に失敗");
            arm.fail_logged = true;
        }
    } else {
        unregister_paste_hotkey();
        arm.armed = false;
        arm.fail_logged = false;
    }
}

fn register_paste_hotkey() -> bool {
    // hWnd=None: WM_HOTKEY をこのスレッドのメッセージキューへ送る
    unsafe {
        RegisterHotKey(
            None,
            PASTE_HOTKEY_ID,
            HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_NOREPEAT.0),
            u32::from(VK_V.0),
        )
        .is_ok()
    }
}

fn unregister_paste_hotkey() {
    let _ = unsafe { UnregisterHotKey(None, PASTE_HOTKEY_ID) };
}

// ======================================================================
// ホットキー処理
// ======================================================================
fn handle_paste_hotkey(state: &AppState, arm: &mut PasteHotkeyArmState) {
    // SendInput の再入で同じホットキーが再度発火しないよう先に解除する
    if arm.armed {
        unregister_paste_hotkey();
        arm.armed = false;
    }

    let _ = super::paste::try_paste_continuous_record_front_standalone(state);

    // RegisterHotKey が V を消費済み。物理 Ctrl は押下中のため V のみ送る
    if !simulate_v_while_ctrl_held() {
        crate::log_warn!("連続記録貼り付けのキー送信に失敗");
    }

    sync_paste_hotkey(state, arm);
}

/// 物理 Ctrl 押下中に `V` を送り、フォアグラウンドアプリへ貼り付けを通す
fn simulate_v_while_ctrl_held() -> bool {
    let inputs = [keyboard_input(VK_V, false), keyboard_input(VK_V, true)];
    let Ok(cbsize) = i32::try_from(mem::size_of::<INPUT>()) else {
        return false;
    };
    let Ok(expected) = u32::try_from(inputs.len()) else {
        return false;
    };
    let sent = unsafe { SendInput(&inputs, cbsize) };
    sent == expected
}

fn keyboard_input(
    vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    key_up: bool,
) -> INPUT {
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

    use crate::tray::state::test_app_state;

    /// 連続記録 OFF ではホットキーを武装しないこと
    #[test]
    fn should_not_arm_when_continuous_record_disabled() {
        let state = test_app_state();
        state.enqueue_continuous_record("item");
        assert!(!should_arm_paste_hotkey(&state));
    }

    /// キュー空ではホットキーを武装しないこと
    #[test]
    fn should_not_arm_when_queue_empty() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        assert!(!should_arm_paste_hotkey(&state));
    }

    /// 連続記録 ON かつキュー非空のときだけ武装すること
    #[test]
    fn should_arm_when_enabled_and_queue_nonempty() {
        let state = test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        state.enqueue_continuous_record("item");
        assert!(should_arm_paste_hotkey(&state));
    }
}

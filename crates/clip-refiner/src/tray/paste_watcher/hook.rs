use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};

use super::super::state::{AppEvent, AppState};

use tao::event_loop::EventLoopProxy;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_QUIT, WM_SYSKEYDOWN,
};

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
                let _ = PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ======================================================================
// フックコンテキスト
// ======================================================================
struct HookContext {
    state: Arc<AppState>,
    stop: Arc<AtomicBool>,
}

thread_local! {
    static HOOK_CTX: RefCell<Option<HookContext>> = const { RefCell::new(None) };
}

// ======================================================================
// スレッド起動
// ======================================================================
/// Windows 向け低レベルキーボードフックで貼り付けを検知する
pub fn start(
    _proxy: EventLoopProxy<AppEvent>,
    state: Arc<AppState>,
    _clipboard_worker: Arc<super::super::worker::ClipboardWorkerHandle>,
) -> PasteWatcherStop {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let thread_id = Arc::new(AtomicU32::new(0));
    let thread_id_for_loop = Arc::clone(&thread_id);
    let thread = thread::spawn(move || run_hook_loop(state, &stop_for_thread, &thread_id_for_loop));
    PasteWatcherStop {
        stop,
        thread: Some(thread),
        thread_id,
    }
}

fn run_hook_loop(state: Arc<AppState>, stop: &Arc<AtomicBool>, thread_id: &Arc<AtomicU32>) {
    thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::Release);

    HOOK_CTX.with(|cell| {
        *cell.borrow_mut() = Some(HookContext {
            state,
            stop: Arc::clone(stop),
        });
    });

    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            std::ptr::null_mut(),
            0,
        )
    };
    if hook.is_null() {
        crate::log_warn!("貼り付け監視フックの登録に失敗");
        HOOK_CTX.with(|cell| *cell.borrow_mut() = None);
        return;
    }

    let mut msg = MSG {
        hwnd: std::ptr::null_mut(),
        message: 0,
        wParam: 0,
        lParam: 0,
        time: 0,
        pt: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
    };

    while !stop.load(Ordering::Relaxed) {
        let result = unsafe { GetMessageW(&raw mut msg, std::ptr::null_mut(), 0, 0) };
        if result == 0 || result == -1 {
            break;
        }
        unsafe {
            let _ = DispatchMessageW(&raw const msg);
        }
    }

    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
    HOOK_CTX.with(|cell| *cell.borrow_mut() = None);
}

// ======================================================================
// フック処理
// ======================================================================
unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION.cast_signed() {
        let Ok(msg) = u32::try_from(wparam) else {
            return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
        };
        if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && ctrl_pressed() {
            let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            if kb.vkCode == u32::from(VK_V) {
                HOOK_CTX.with(|cell| {
                    if let Some(ctx) = cell.borrow().as_ref()
                        && !ctx.stop.load(Ordering::Relaxed)
                    {
                        let _ =
                            super::paste::try_paste_continuous_record_front_standalone(&ctx.state);
                    }
                });
            }
        }
    }

    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

fn ctrl_pressed() -> bool {
    key_pressed(VK_LCONTROL) || key_pressed(VK_RCONTROL) || key_pressed(VK_CONTROL)
}

fn key_pressed(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(vk)).cast_unsigned() & 0x8000 != 0 }
}

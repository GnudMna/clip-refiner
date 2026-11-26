//! 更新画面 (Windows 標準の `TaskDialog`)
//!
//! 画面は主スレッドで動かし、更新の進行は別スレッドから `DialogUi` を通してページを切り替える
//! `TaskDialogIndirect` は Common Controls v6 にしか無いため、実行時に取得する

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{ERROR_PROC_NOT_FOUND, HWND, LPARAM, S_FALSE, S_OK, WPARAM};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_FLAGS, LoadLibraryExW,
};
use windows::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOG_ELEMENTS, TASKDIALOG_FLAGS, TASKDIALOG_MESSAGES,
    TASKDIALOG_NOTIFICATIONS, TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TD_ERROR_ICON,
    TD_INFORMATION_ICON, TD_WARNING_ICON, TDE_CONTENT, TDF_ALLOW_DIALOG_CANCELLATION,
    TDF_CALLBACK_TIMER, TDF_CAN_BE_MINIMIZED, TDF_SHOW_MARQUEE_PROGRESS_BAR, TDF_SHOW_PROGRESS_BAR,
    TDM_ENABLE_BUTTON, TDM_NAVIGATE_PAGE, TDM_SET_PROGRESS_BAR_MARQUEE, TDM_SET_PROGRESS_BAR_POS,
    TDM_UPDATE_ELEMENT_TEXT, TDN_BUTTON_CLICKED, TDN_CREATED, TDN_DIALOG_CONSTRUCTED,
    TDN_NAVIGATED, TDN_TIMER,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    FLASHW_ALL, FLASHW_TIMERNOFG, FLASHWINFO, FindWindowW, FlashWindowEx, HICON, ICON_BIG,
    ICON_SMALL, IDCANCEL, IMAGE_ICON, IsIconic, LR_DEFAULTSIZE, LR_SHARED, LoadImageW, SW_RESTORE,
    SW_SHOWNORMAL, SendMessageW, SetForegroundWindow, ShowWindow, WM_SETICON,
};
use windows::core::{BOOL, HRESULT, HSTRING, PCWSTR, s, w};

use super::flow::Ui;
use super::page::{Action, Dismiss, Icon, Page, PageSpec, Progress, percent, progress_text};

// ======================================================================
// 定数
// ======================================================================
/// ウィンドウタイトル (起動済みの画面を探すときにも使う)
const WINDOW_TITLE: PCWSTR = w!("ClipRefiner の更新");
/// `TaskDialog` のウィンドウクラス
const DIALOG_CLASS: PCWSTR = w!("#32770");
/// 実行ファイルに埋め込んだアイコンのリソース番号 (`winresource` の既定)
const APP_ICON_ID: usize = 1;
/// 「更新」ボタンの ID
const ID_UPDATE: i32 = 1001;
/// 「起動」ボタンの ID
const ID_START: i32 = 1002;
/// 「ログを開く」ボタンの ID
const ID_OPEN_LOG: i32 = 1003;
/// 閉じるボタンの ID (`×` と `Esc` も同じ ID で届く)
const ID_DISMISS: i32 = IDCANCEL.0;
/// 流れる進捗バーの更新間隔 (ミリ秒)
const MARQUEE_INTERVAL_MS: isize = 30;

/// `TaskDialogIndirect` の型
type TaskDialogIndirectFn =
    unsafe extern "system" fn(*const TASKDIALOGCONFIG, *mut i32, *mut i32, *mut BOOL) -> HRESULT;

// ======================================================================
// 段階
// ======================================================================
/// 打ち切りとインストール開始の、どちらが先かを 1 度だけ決める
struct Gate(AtomicU8);

impl Gate {
    /// まだ打ち切れる
    const OPEN: u8 = 0;
    /// インストールの段階に入った
    const COMMITTED: u8 = 1;
    /// 打ち切られた
    const ABORTED: u8 = 2;

    fn new() -> Self {
        Self(AtomicU8::new(Self::OPEN))
    }

    fn try_commit(&self) -> bool {
        self.transition(Self::COMMITTED)
    }

    fn try_abort(&self) -> bool {
        self.transition(Self::ABORTED)
    }

    fn is_aborted(&self) -> bool {
        self.0.load(Ordering::SeqCst) == Self::ABORTED
    }

    fn transition(&self, to: u8) -> bool {
        self.0
            .compare_exchange(Self::OPEN, to, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

// ======================================================================
// 進行側の窓口
// ======================================================================
/// 画面と進行で共有する状態
struct Shared {
    /// 表示待ちのページと、操作を待つページか
    pages: Mutex<VecDeque<(Page, bool)>>,
    /// 最新のダウンロード進捗 (受信済み, 全体)
    progress: Mutex<Option<(u64, u64)>>,
    gate: Gate,
    cancel: AtomicBool,
}

impl Shared {
    fn new() -> Self {
        Self {
            pages: Mutex::default(),
            progress: Mutex::default(),
            gate: Gate::new(),
            cancel: AtomicBool::new(false),
        }
    }
}

/// 更新の進行から画面を操作する窓口
pub(super) struct DialogUi {
    shared: Arc<Shared>,
    actions: Receiver<Action>,
}

impl DialogUi {
    fn push(&self, page: Page, ask: bool) {
        // 前のページの進捗が、次に表示するページへ反映されないようにする
        *lock(&self.shared.progress) = None;
        lock(&self.shared.pages).push_back((page, ask));
    }
}

impl Ui for DialogUi {
    fn show(&self, page: Page) {
        self.push(page, false);
    }

    fn ask(&self, page: Page) -> Action {
        self.push(page, true);
        // 画面が失われた場合は、閉じられたものとして進める
        self.actions.recv().unwrap_or(Action::Close)
    }

    fn progress(&self, received: u64, total: u64) {
        *lock(&self.shared.progress) = Some((received, total));
    }

    fn begin_apply(&self) -> bool {
        self.shared.gate.try_commit()
    }

    fn cancel_requested(&self) -> bool {
        self.shared.cancel.load(Ordering::SeqCst)
    }
}

/// 毒化を無視してロックする
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ======================================================================
// 表示
// ======================================================================
/// 更新画面を表示し、閉じられるまで待つ
///
/// 画面を表示できてから `work` を始めるため、`Err` のときは何も実行していない
///
/// # Arguments
/// * `first` - 最初に表示するページ
/// * `log_dir` - 「ログを開く」で開くフォルダ
/// * `work` - 別スレッドで実行する更新の進行
///
/// # Returns
/// * `windows::core::Result<()>` - 画面を表示できない場合はエラー
pub(super) fn run(
    first: &Page,
    log_dir: &Path,
    work: impl FnOnce(&DialogUi) + Send + 'static,
) -> windows::core::Result<()> {
    let task_dialog = load_task_dialog()?;
    let shared = Arc::new(Shared::new());
    let (sender, actions) = channel();
    let ui = DialogUi {
        shared: Arc::clone(&shared),
        actions,
    };
    let dialog = Dialog {
        shared: Arc::clone(&shared),
        sender,
        log_dir: log_dir.to_path_buf(),
        pages: RefCell::default(),
        dismiss: Cell::new(Dismiss::Abort),
        asking: Cell::new(false),
        is_result: Cell::new(false),
        pending_work: RefCell::new(Some(Box::new(move || work(&ui)))),
        worker: RefCell::default(),
    };

    let config = dialog.render(first, false);
    // SAFETY: `config` と、それが指す文字列・ボタン・`dialog` は、画面が閉じるまで `dialog` とともに生存する
    let result = unsafe {
        task_dialog(
            config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    let worker = dialog.worker.take();
    // 操作を待っている進行へ、画面が閉じたことを伝える
    drop(dialog);
    if let Some(worker) = worker
        && !shared.gate.is_aborted()
    {
        let _ = worker.join();
    }
    result.ok()
}

/// 起動済みの更新画面を前面に出す
pub(super) fn activate_existing() {
    // SAFETY: 引数は NUL 終端の静的文字列で、得たハンドルは無効でも各 API が失敗を返すだけ
    unsafe {
        let Ok(hwnd) = FindWindowW(DIALOG_CLASS, WINDOW_TITLE) else {
            return;
        };
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Common Controls v6 の `TaskDialogIndirect` を取得する
///
/// マニフェストが効いていない環境では見つからず、エラーになる
fn load_task_dialog() -> windows::core::Result<TaskDialogIndirectFn> {
    // SAFETY: `comctl32.dll` は Windows が管理する既知の DLL で、引数は NUL 終端の静的文字列
    // 取得した関数は公開されている `TaskDialogIndirect` で、型は Windows SDK の宣言と一致する
    unsafe {
        let module = LoadLibraryExW(w!("comctl32.dll"), None, LOAD_LIBRARY_FLAGS(0))?;
        let proc = GetProcAddress(module, s!("TaskDialogIndirect"))
            .ok_or_else(|| windows::core::Error::from(ERROR_PROC_NOT_FOUND.to_hresult()))?;
        Ok(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            TaskDialogIndirectFn,
        >(proc))
    }
}

// ======================================================================
// ページの組み立て
// ======================================================================
/// `TaskDialog` が参照する、1 ページ分の文字列と設定
struct RenderedPage {
    config: TASKDIALOGCONFIG,
    _texts: [HSTRING; 2],
    _labels: Vec<HSTRING>,
    _buttons: Vec<TASKDIALOG_BUTTON>,
}

/// 画面の状態 (主スレッドだけで使う)
struct Dialog {
    shared: Arc<Shared>,
    sender: Sender<Action>,
    log_dir: PathBuf,
    /// 表示したページ。画面が文字列を参照し続けるため、閉じるまで保持する
    pages: RefCell<Vec<Pin<Box<RenderedPage>>>>,
    /// 表示中のページの閉じ方
    dismiss: Cell<Dismiss>,
    /// 表示中のページが操作を待っているか
    asking: Cell<bool>,
    /// 表示中のページが更新の結果か
    is_result: Cell<bool>,
    /// 画面の表示後に始める進行
    pending_work: RefCell<Option<Box<dyn FnOnce() + Send>>>,
    worker: RefCell<Option<JoinHandle<()>>>,
}

impl Dialog {
    /// ページの設定を作って保持し、そのポインタを返す
    fn render(&self, page: &Page, ask: bool) -> *const TASKDIALOGCONFIG {
        let spec = page.spec();
        self.dismiss.set(spec.dismiss);
        self.asking.set(ask);
        self.is_result.set(spec.is_result);

        let instruction = HSTRING::from(&spec.instruction);
        let content = HSTRING::from(&spec.content);
        let (ids, labels): (Vec<_>, Vec<_>) = button_labels(&spec)
            .into_iter()
            .map(|(id, label)| (id, HSTRING::from(label)))
            .unzip();
        let buttons: Vec<_> = ids
            .iter()
            .zip(&labels)
            .map(|(id, label)| TASKDIALOG_BUTTON {
                nButtonID: *id,
                pszButtonText: PCWSTR(label.as_ptr()),
            })
            .collect();

        let config = TASKDIALOGCONFIG {
            cbSize: u32::try_from(size_of::<TASKDIALOGCONFIG>()).unwrap_or_default(),
            dwFlags: flags(&spec),
            pszWindowTitle: WINDOW_TITLE,
            Anonymous1: TASKDIALOGCONFIG_0 {
                pszMainIcon: icon(spec.icon),
            },
            pszMainInstruction: PCWSTR(instruction.as_ptr()),
            pszContent: PCWSTR(content.as_ptr()),
            cButtons: u32::try_from(buttons.len()).unwrap_or_default(),
            pButtons: buttons.as_ptr(),
            nDefaultButton: ids[0],
            pfCallback: Some(callback),
            lpCallbackData: std::ptr::from_ref(self).expose_provenance().cast_signed(),
            ..TASKDIALOGCONFIG::default()
        };
        let rendered = Box::pin(RenderedPage {
            config,
            _texts: [instruction, content],
            _labels: labels,
            _buttons: buttons,
        });
        // `Box` の中身は `pages` の伸長でも移動しない
        let pointer = &raw const rendered.config;
        self.pages.borrow_mut().push(rendered);
        pointer
    }

    /// 画面からの通知を処理する
    fn handle(
        &self,
        hwnd: HWND,
        notification: TASKDIALOG_NOTIFICATIONS,
        wparam: WPARAM,
    ) -> HRESULT {
        match notification {
            TDN_DIALOG_CONSTRUCTED => set_window_icon(hwnd),
            TDN_CREATED => {
                self.page_shown(hwnd);
                self.start_work();
            }
            TDN_NAVIGATED => self.page_shown(hwnd),
            TDN_TIMER => self.apply_pending(hwnd),
            TDN_BUTTON_CLICKED => {
                let id = i32::try_from(wparam.0).unwrap_or(ID_DISMISS);
                return if self.button_clicked(hwnd, id) {
                    S_OK
                } else {
                    S_FALSE
                };
            }
            _ => {}
        }
        S_OK
    }

    /// 画面を表示できたので、更新の進行を始める
    fn start_work(&self) {
        if let Some(work) = self.pending_work.take() {
            self.worker.replace(Some(thread::spawn(work)));
        }
    }

    /// 進行から届いたページと進捗を反映する
    fn apply_pending(&self, hwnd: HWND) {
        // 表示が追い付かなかったページは飛ばし、最後のものだけを表示する
        let next = lock(&self.shared.pages).drain(..).next_back();
        if let Some((page, ask)) = next {
            let config = self.render(&page, ask);
            send(
                hwnd,
                TDM_NAVIGATE_PAGE,
                0,
                config.expose_provenance().cast_signed(),
            );
        }

        let progress = lock(&self.shared.progress).take();
        if let Some((received, total)) = progress {
            send(
                hwnd,
                TDM_SET_PROGRESS_BAR_POS,
                usize::from(percent(received, total)),
                0,
            );
            set_text(hwnd, TDE_CONTENT, &progress_text(received, total));
        }
    }

    /// ページが表示された直後の仕上げをする
    fn page_shown(&self, hwnd: HWND) {
        send(hwnd, TDM_SET_PROGRESS_BAR_MARQUEE, 1, MARQUEE_INTERVAL_MS);
        if self.dismiss.get() == Dismiss::Denied {
            enable_dismiss(hwnd, false);
        }
        if self.is_result.get() {
            call_attention(hwnd);
        }
    }

    /// ボタンの操作を処理し、画面を閉じてよければ `true` を返す
    fn button_clicked(&self, hwnd: HWND, id: i32) -> bool {
        match id {
            ID_OPEN_LOG => open_folder(hwnd, &self.log_dir),
            ID_UPDATE => self.reply(Action::Update),
            ID_START => self.reply(Action::Start),
            _ => return self.dismiss_clicked(hwnd),
        }
        false
    }

    /// 閉じるボタンの操作を処理し、画面を閉じてよければ `true` を返す
    fn dismiss_clicked(&self, hwnd: HWND) -> bool {
        match self.dismiss.get() {
            Dismiss::Close if self.asking.get() => {
                self.reply(Action::Close);
                true
            }
            // 別の操作を選んだ直後で、次のページをまだ表示していない
            Dismiss::Close | Dismiss::Abort => self.shared.gate.try_abort(),
            Dismiss::RequestCancel => {
                self.shared.cancel.store(true, Ordering::SeqCst);
                enable_dismiss(hwnd, false);
                false
            }
            Dismiss::Denied => false,
        }
    }

    /// 待っている進行へ操作を伝える。1 ページにつき 1 度だけ伝える
    fn reply(&self, action: Action) {
        if self.asking.replace(false) {
            let _ = self.sender.send(action);
        }
    }
}

/// `TaskDialog` からの通知を受け取る
///
/// # Safety
/// `data` は、画面が閉じるまで生存する `Dialog` を指すこと
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: TASKDIALOG_NOTIFICATIONS,
    wparam: WPARAM,
    _lparam: LPARAM,
    data: isize,
) -> HRESULT {
    // SAFETY: `data` は `render` が渡した `Dialog` のアドレスで、`run` が画面を閉じるまで保持する
    // 通知は画面を作った主スレッドだけに届く
    let dialog = unsafe { &*std::ptr::with_exposed_provenance::<Dialog>(data.cast_unsigned()) };
    dialog.handle(hwnd, notification, wparam)
}

/// ボタンの ID と表示名を左から順に返す。末尾は必ず閉じるボタンになる
fn button_labels(spec: &PageSpec) -> Vec<(i32, &'static str)> {
    let mut buttons: Vec<_> = spec
        .buttons
        .iter()
        .map(|(action, label)| (button_id(*action), *label))
        .collect();
    buttons.push((ID_DISMISS, spec.dismiss.label()));
    buttons
}

/// 操作に対応するボタンの ID を返す
fn button_id(action: Action) -> i32 {
    match action {
        Action::Update => ID_UPDATE,
        Action::Start => ID_START,
        Action::OpenLog => ID_OPEN_LOG,
        Action::Close => ID_DISMISS,
    }
}

/// ページの表示内容から画面のフラグを決める
fn flags(spec: &PageSpec) -> TASKDIALOG_FLAGS {
    let mut flags = TDF_CALLBACK_TIMER | TDF_CAN_BE_MINIMIZED;
    if spec.dismiss != Dismiss::Denied {
        flags |= TDF_ALLOW_DIALOG_CANCELLATION;
    }
    match spec.progress {
        Progress::Hidden => {}
        Progress::Percent => flags |= TDF_SHOW_PROGRESS_BAR,
        Progress::Marquee => flags |= TDF_SHOW_MARQUEE_PROGRESS_BAR,
    }
    flags
}

/// アイコンのリソースを返す
fn icon(icon: Icon) -> PCWSTR {
    match icon {
        Icon::Information => TD_INFORMATION_ICON,
        Icon::Warning => TD_WARNING_ICON,
        Icon::Error => TD_ERROR_ICON,
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
/// 画面へメッセージを送る
fn send(hwnd: HWND, message: TASKDIALOG_MESSAGES, wparam: usize, lparam: isize) {
    // SAFETY: `hwnd` は通知元の画面で、`lparam` が指す内容は呼び出し元が処理の完了まで保持する
    unsafe {
        SendMessageW(
            hwnd,
            message.0.cast_unsigned(),
            Some(WPARAM(wparam)),
            Some(LPARAM(lparam)),
        );
    }
}

/// 表示中の文言を差し替える
fn set_text(hwnd: HWND, element: TASKDIALOG_ELEMENTS, text: &str) {
    let text = HSTRING::from(text);
    send(
        hwnd,
        TDM_UPDATE_ELEMENT_TEXT,
        usize::try_from(element.0).unwrap_or_default(),
        text.as_ptr().expose_provenance().cast_signed(),
    );
}

/// 閉じるボタンの有効・無効を切り替える
fn enable_dismiss(hwnd: HWND, enabled: bool) {
    send(
        hwnd,
        TDM_ENABLE_BUTTON,
        usize::try_from(ID_DISMISS).unwrap_or_default(),
        isize::from(enabled),
    );
}

/// タイトルバーとタスクバーのアイコンを設定する
fn set_window_icon(hwnd: HWND) {
    // SAFETY: 自身の実行ファイルのリソースを共有で読み込み、画面へ設定する。共有のため解放は要らない
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let Ok(handle) = LoadImageW(
            Some(module.into()),
            PCWSTR(std::ptr::without_provenance(APP_ICON_ID)),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        ) else {
            return;
        };
        let icon = HICON(handle.0);
        for size in [ICON_BIG, ICON_SMALL] {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(usize::try_from(size).unwrap_or_default())),
                Some(LPARAM(icon.0.expose_provenance().cast_signed())),
            );
        }
    }
}

/// 結果に気付けるよう、最小化を戻し、前面でなければタスクバーを点滅させる
fn call_attention(hwnd: HWND) {
    // SAFETY: `hwnd` は表示中の画面で、`info` は呼び出し中生存する
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let info = FLASHWINFO {
            cbSize: u32::try_from(size_of::<FLASHWINFO>()).unwrap_or_default(),
            hwnd,
            dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        let _ = FlashWindowEx(&raw const info);
    }
}

/// フォルダをエクスプローラーで開く
fn open_folder(hwnd: HWND, dir: &Path) {
    let dir = HSTRING::from(dir.as_os_str());
    // SAFETY: 文字列はいずれも NUL 終端で、呼び出し完了まで生存する
    let result = unsafe {
        ShellExecuteW(
            Some(hwnd),
            w!("open"),
            &dir,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // 32 以下は失敗を表す
    if result.0.cast::<c_void>().addr() <= 32 {
        tracing::warn!("ログのフォルダを開けない");
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn test_dialog() -> (Dialog, DialogUi) {
        let shared = Arc::new(Shared::new());
        let (sender, actions) = channel();
        let dialog = Dialog {
            shared: Arc::clone(&shared),
            sender,
            log_dir: PathBuf::new(),
            pages: RefCell::default(),
            dismiss: Cell::new(Dismiss::Abort),
            asking: Cell::new(false),
            is_result: Cell::new(false),
            pending_work: RefCell::default(),
            worker: RefCell::default(),
        };
        (dialog, DialogUi { shared, actions })
    }

    fn available() -> Page {
        Page::Available {
            current: "4.0.1".to_string(),
            latest: "4.1.0".to_string(),
            size: 10,
        }
    }

    /// 打ち切りとインストール開始は、先に起きた方だけが成立すること
    #[test]
    fn gate_accepts_only_first_transition() {
        let gate = Gate::new();
        assert!(gate.try_commit());
        assert!(!gate.try_abort());
        assert!(!gate.is_aborted());

        let gate = Gate::new();
        assert!(gate.try_abort());
        assert!(!gate.try_commit());
        assert!(gate.is_aborted());
    }

    /// ダウンロード中は閉じられ、インストールの段階に入ると閉じられないこと
    #[test]
    fn abort_is_refused_after_commit() {
        let (dialog, ui) = test_dialog();
        dialog.render(&Page::Checking, false);
        assert!(ui.begin_apply());
        assert!(!dialog.button_clicked(HWND::default(), ID_DISMISS));

        let (dialog, ui) = test_dialog();
        dialog.render(&Page::Checking, false);
        assert!(dialog.button_clicked(HWND::default(), ID_DISMISS));
        assert!(!ui.begin_apply());
    }

    /// 終了待ちの中止は進行へ伝わり、画面は閉じないこと
    #[test]
    fn cancel_request_keeps_dialog_open() {
        let (dialog, ui) = test_dialog();
        dialog.render(&Page::WaitingExit, false);

        assert!(!dialog.button_clicked(HWND::default(), ID_DISMISS));
        assert!(ui.cancel_requested());
    }

    /// 1 ページの操作は 1 度だけ伝わり、続けて押しても次のページへ残らないこと
    #[test]
    fn action_is_sent_once_per_page() {
        let (dialog, ui) = test_dialog();
        dialog.render(&available(), true);

        assert!(!dialog.button_clicked(HWND::default(), ID_UPDATE));
        assert!(!dialog.button_clicked(HWND::default(), ID_UPDATE));
        assert_eq!(ui.actions.try_recv(), Ok(Action::Update));
        assert!(ui.actions.try_recv().is_err());
    }

    /// 更新を選んだ直後に閉じると、ダウンロードを打ち切ること
    #[test]
    fn closing_after_update_aborts() {
        let (dialog, ui) = test_dialog();
        dialog.render(&available(), true);
        dialog.button_clicked(HWND::default(), ID_UPDATE);

        assert!(dialog.button_clicked(HWND::default(), ID_DISMISS));
        assert!(!ui.begin_apply());
    }

    /// インストール中は閉じられないこと
    #[test]
    fn installing_page_cannot_be_closed() {
        let (dialog, _ui) = test_dialog();
        dialog.render(
            &Page::Installing {
                latest: "4.1.0".to_string(),
            },
            false,
        );
        assert!(!dialog.button_clicked(HWND::default(), ID_DISMISS));
    }

    /// 画面が失われたら、操作待ちは閉じたものとして戻ること
    #[test]
    fn ask_returns_close_when_dialog_is_gone() {
        let (dialog, ui) = test_dialog();
        drop(dialog);
        assert_eq!(ui.ask(available()), Action::Close);
    }

    /// 閉じるボタンを末尾に持ち、閉じられないページだけ `×` を受け付けないこと
    #[test]
    fn page_has_dismiss_button() {
        assert_eq!(
            button_labels(&available().spec()),
            [(ID_UPDATE, "更新"), (ID_DISMISS, "閉じる")]
        );

        let cancelable = |page: &Page| flags(&page.spec()).0 & TDF_ALLOW_DIALOG_CANCELLATION.0 != 0;
        assert!(cancelable(&available()));
        assert!(!cancelable(&Page::Starting));
    }
}

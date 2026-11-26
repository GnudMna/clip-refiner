//! 常駐アプリと更新プログラムの間で使う名前付きイベント
//!
//! 名前はログオンセッション内 (`Local\`) に限り、既定の DACL により同じ利用者だけが開ける

use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::time::Duration;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_EVENT, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    CreateEventW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, ResetEvent, SetEvent,
    WaitForMultipleObjects,
};
use windows::core::HSTRING;

// ======================================================================
// 定数
// ======================================================================
/// `WaitForMultipleObjects` でプロセスの終了を示す戻り値 (イベントの次の位置)
const WAIT_PROCESS: WAIT_EVENT = WAIT_EVENT(WAIT_OBJECT_0.0 + 1);

// ======================================================================
// 型
// ======================================================================
/// 名前付きイベント
pub(crate) struct NamedEvent {
    handle: OwnedHandle,
    already_exists: bool,
}

/// イベントとプロセスの待ち合わせ結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventWait {
    /// イベントが通知された
    Signaled,
    /// 通知より先にプロセスが終了した
    ProcessExited,
    /// 制限時間を過ぎた
    TimedOut,
}

impl NamedEvent {
    /// 名前付きイベントを作る (同名があればそれを開く)
    ///
    /// # Arguments
    /// * `name` - イベント名
    /// * `manual_reset` - 待機で消費されない通知にする場合は `true`
    ///
    /// # Returns
    /// * `windows::core::Result<Self>` - 作成または既存を開けた場合のイベント
    pub(crate) fn create(name: &str, manual_reset: bool) -> windows::core::Result<Self> {
        let name = HSTRING::from(name);
        // SAFETY: `name` は NUL 終端の UTF-16 で、呼び出し完了まで生存する
        let handle = unsafe { CreateEventW(None, manual_reset, false, &name) }?;
        // SAFETY: 直前の API 呼び出しのエラーコードを読むだけで、間に他の Win32 呼び出しを挟まない
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        Ok(Self {
            // SAFETY: `CreateEventW` が成功して返したハンドルで、ここで唯一の所有者になる
            handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
            already_exists,
        })
    }

    /// 作成時に同名のイベントが既に存在していたか
    ///
    /// # Returns
    /// * `bool` - 別のプロセスなどが先に作成していた場合は `true`
    pub(crate) fn already_exists(&self) -> bool {
        self.already_exists
    }

    /// 受け取り済みの通知を取り消す
    ///
    /// # Returns
    /// * `windows::core::Result<()>` - 取り消せない場合はエラー
    pub(crate) fn reset(&self) -> windows::core::Result<()> {
        // SAFETY: ハンドルは `self` が保持し、作成時の既定のアクセス権で変更できる
        unsafe { ResetEvent(HANDLE(self.handle.as_raw_handle())) }
    }

    /// イベントの通知またはプロセスの終了を待つ
    ///
    /// 両方が成立している場合は通知を優先する
    ///
    /// # Arguments
    /// * `process` - 併せて待つプロセス
    /// * `timeout` - 待機の上限 (`None` は無期限)
    ///
    /// # Returns
    /// * `windows::core::Result<EventWait>` - 先に成立したもの
    pub(crate) fn wait_with_process(
        &self,
        process: BorrowedHandle<'_>,
        timeout: Option<Duration>,
    ) -> windows::core::Result<EventWait> {
        let handles = [
            HANDLE(self.handle.as_raw_handle()),
            HANDLE(process.as_raw_handle()),
        ];
        // SAFETY: 2 つのハンドルは `self` と `process` が呼び出し完了まで保持している
        let wait = unsafe { WaitForMultipleObjects(&handles, false, timeout_millis(timeout)) };
        if wait == WAIT_OBJECT_0 {
            Ok(EventWait::Signaled)
        } else if wait == WAIT_PROCESS {
            Ok(EventWait::ProcessExited)
        } else if wait == WAIT_TIMEOUT {
            Ok(EventWait::TimedOut)
        } else {
            Err(windows::core::Error::from_thread())
        }
    }
}

// ======================================================================
// 通知
// ======================================================================
/// 既存の名前付きイベントを通知する
///
/// # Arguments
/// * `name` - イベント名
///
/// # Returns
/// * `windows::core::Result<()>` - イベントが無い場合や通知できない場合はエラー
pub(crate) fn signal(name: &str) -> windows::core::Result<()> {
    let name = HSTRING::from(name);
    // SAFETY: `name` は NUL 終端の UTF-16 で、呼び出し完了まで生存する
    let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, &name) }?;
    // SAFETY: `OpenEventW` が成功して返したハンドルで、ここで唯一の所有者になる
    let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    // SAFETY: `handle` は `EVENT_MODIFY_STATE` 付きで開いた生存中のハンドル
    unsafe { SetEvent(HANDLE(handle.as_raw_handle())) }
}

/// 待機の上限をミリ秒へ変換する (`None` は無期限)
fn timeout_millis(timeout: Option<Duration>) -> u32 {
    match timeout {
        Some(timeout) => u32::try_from(timeout.as_millis()).unwrap_or(INFINITE - 1),
        None => INFINITE,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::os::windows::io::AsHandle;
    use std::process::{Child, Command, Stdio};

    use super::*;

    fn unique_name(label: &str) -> String {
        format!(
            r"Local\clip-refiner-test-event-{label}-{}",
            std::process::id()
        )
    }

    /// 数秒動き続けるプロセスを起動する
    fn spawn_sleeper() -> Child {
        Command::new("ping")
            .args(["127.0.0.1", "-n", "6"])
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn")
    }

    /// 通知されたイベントをプロセスの終了より先に検知すること
    #[test]
    fn wait_reports_signal() {
        let name = unique_name("signal");
        let event = NamedEvent::create(&name, true).expect("create");
        let mut child = spawn_sleeper();

        assert_eq!(
            event
                .wait_with_process(child.as_handle(), Some(Duration::from_millis(20)))
                .expect("wait"),
            EventWait::TimedOut
        );
        signal(&name).expect("signal");
        assert_eq!(
            event
                .wait_with_process(child.as_handle(), None)
                .expect("wait"),
            EventWait::Signaled
        );

        // 取り消した後は、再び通知されるまで成立しない
        event.reset().expect("reset");
        assert_eq!(
            event
                .wait_with_process(child.as_handle(), Some(Duration::from_millis(20)))
                .expect("wait"),
            EventWait::TimedOut
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    /// 通知の前にプロセスが終了したことを検知すること
    #[test]
    fn wait_reports_process_exit() {
        let event = NamedEvent::create(&unique_name("exit"), true).expect("create");
        let mut child = spawn_sleeper();
        child.kill().expect("kill");
        child.wait().expect("wait child");

        assert_eq!(
            event
                .wait_with_process(child.as_handle(), None)
                .expect("wait"),
            EventWait::ProcessExited
        );
    }

    /// 2 つ目の作成で同名のイベントの存在を検知すること
    #[test]
    fn second_create_reports_existing() {
        let name = unique_name("exists");
        let first = NamedEvent::create(&name, true).expect("first");
        let second = NamedEvent::create(&name, true).expect("second");

        assert!(!first.already_exists());
        assert!(second.already_exists());
    }

    /// 存在しないイベントへの通知は失敗すること
    #[test]
    fn signal_fails_when_event_is_missing() {
        assert!(signal(&unique_name("missing")).is_err());
    }
}

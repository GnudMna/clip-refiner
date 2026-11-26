//! 常駐アプリ (`ClipRefiner.exe`) の終了と起動
//!
//! 終了の依頼・終了待ち・起動と、起動完了の確認を担う

use std::os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use clip_refiner_core::consts::APP_ID;
use windows::Win32::Foundation::{ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    OpenMutexW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
};
use windows::core::HSTRING;

use super::error::UpdateError;
use super::named_event::{self, EventWait, NamedEvent};
use super::replace::{portable_exe_name, process_image_path};
use super::session::{exit_request_event_name, startup_complete_event_name};

// ======================================================================
// 定数
// ======================================================================
/// 起動完了を待つ上限
///
/// 更新直後はウイルス対策ソフトの検査で起動が遅くなることがある
const START_TIMEOUT: Duration = Duration::from_mins(1);
/// 対象のプロセスの終了後、ほかの常駐アプリの有無を調べ直す間隔
const INSTANCE_POLL_INTERVAL: Duration = Duration::from_millis(200);

// ======================================================================
// 終了
// ======================================================================
/// 更新前に終了してもらう常駐アプリ
#[derive(Debug)]
pub(crate) struct ResidentProcess {
    pid: u32,
    /// 開いたときに既に終了していた場合は `None`
    handle: Option<OwnedHandle>,
    /// 常駐アプリが多重起動の防止に使う mutex の名前
    instance_name: &'static str,
}

impl ResidentProcess {
    /// 指定した PID の常駐アプリを開く
    ///
    /// 生存中なら、実行ファイルが `install_dir` の `ClipRefiner.exe` であることを要求する
    /// 終了後に PID が再利用されても取り違えないよう、ハンドルを持ち続ける
    ///
    /// # Arguments
    /// * `pid` - 常駐アプリの PID
    /// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
    ///
    /// # Returns
    /// * `Result<Self, UpdateError>` - 開けた場合、または既に終了していた場合のインスタンス
    pub(crate) fn open(pid: u32, install_dir: &Path) -> Result<Self, UpdateError> {
        Self::open_with_instance(pid, install_dir, APP_ID)
    }

    /// 多重起動の防止に使う mutex の名前を指定して開く
    fn open_with_instance(
        pid: u32,
        install_dir: &Path,
        instance_name: &'static str,
    ) -> Result<Self, UpdateError> {
        if pid == 0 || pid == std::process::id() {
            tracing::warn!("待ち受け PID が不正: {pid}");
            return Err(UpdateError::WaitPidInvalid);
        }

        // SAFETY: ポインタを受け取らず、成功時に返るハンドルは直後に所有権を移す
        let opened = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        };
        let handle = match opened {
            // SAFETY: `OpenProcess` が成功して返したハンドルで、ここで唯一の所有者になる
            Ok(handle) => unsafe { OwnedHandle::from_raw_handle(handle.0) },
            Err(err) if err.code() == ERROR_INVALID_PARAMETER.into() => {
                tracing::info!("待ち受け PID は既に終了している: {pid}");
                return Ok(Self {
                    pid,
                    handle: None,
                    instance_name,
                });
            }
            Err(err) => {
                tracing::warn!("常駐アプリのプロセスを開けない: {err}");
                return Err(UpdateError::WaitPidFailed);
            }
        };

        let image = process_image_path(HANDLE(handle.as_raw_handle()));
        if !image.is_some_and(|image| same_file(&image, &install_dir.join(portable_exe_name()))) {
            tracing::warn!("待ち受けプロセスが実行フォルダの ClipRefiner ではない: pid={pid}");
            return Err(UpdateError::WaitPidNotClipRefiner);
        }
        Ok(Self {
            pid,
            handle: Some(handle),
            instance_name,
        })
    }

    /// 終了を依頼する
    ///
    /// # Returns
    /// * `Result<(), UpdateError>` - 依頼できた場合と、既に終了している場合は `Ok(())`
    pub(crate) fn request_exit(&self) -> Result<(), UpdateError> {
        if self.wait_exit(Duration::ZERO)? {
            return Ok(());
        }
        named_event::signal(&exit_request_event_name(self.pid)).map_err(|err| {
            tracing::warn!("常駐アプリへの終了依頼に失敗: {err}");
            UpdateError::ExitRequest
        })
    }

    /// 常駐アプリが動作していない状態になるのを待つ
    ///
    /// 更新画面を開いたままアプリが起動し直された場合は、対象のプロセスが終了していても、
    /// 別の常駐アプリが実行ファイルを使っている。その間は終了していないものとして扱う
    ///
    /// # Arguments
    /// * `timeout` - 待機の上限
    ///
    /// # Returns
    /// * `Result<bool, UpdateError>` - 動作していなければ `true`、上限まで待っても動作していれば `false`
    pub(crate) fn wait_exit(&self, timeout: Duration) -> Result<bool, UpdateError> {
        if !self.wait_process(timeout)? {
            return Ok(false);
        }
        if !instance_exists(self.instance_name) {
            return Ok(true);
        }
        thread::sleep(timeout.min(INSTANCE_POLL_INTERVAL));
        Ok(!instance_exists(self.instance_name))
    }

    /// 対象のプロセスの終了を待つ
    fn wait_process(&self, timeout: Duration) -> Result<bool, UpdateError> {
        let Some(handle) = &self.handle else {
            return Ok(true);
        };
        let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX - 1);
        // SAFETY: `handle` は `PROCESS_SYNCHRONIZE` 付きで開き、`self` が保持している
        let wait = unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), millis) };
        if wait == WAIT_OBJECT_0 {
            Ok(true)
        } else if wait == WAIT_TIMEOUT {
            Ok(false)
        } else {
            tracing::warn!("常駐アプリの終了待ちに失敗: {wait:?}");
            Err(UpdateError::WaitPidFailed)
        }
    }
}

/// 常駐アプリが保持する、多重起動の防止の mutex が存在するか
fn instance_exists(name: &str) -> bool {
    let name = HSTRING::from(name);
    // SAFETY: `name` は NUL 終端の UTF-16 で、呼び出し完了まで生存する
    let Ok(handle) = (unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) }) else {
        return false;
    };
    // SAFETY: `OpenMutexW` が成功して返したハンドルで、ここで唯一の所有者になり、直後に閉じる
    drop(unsafe { OwnedHandle::from_raw_handle(handle.0) });
    true
}

/// 2 つのパスが同じファイルを指すか
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}

// ======================================================================
// 起動
// ======================================================================
/// 常駐アプリの起動と、起動完了の確認
pub(crate) struct ResidentStarter {
    exe: PathBuf,
    install_dir: PathBuf,
    started: Option<NamedEvent>,
    child: Option<Child>,
}

impl ResidentStarter {
    /// 起動前の状態を作る
    ///
    /// # Arguments
    /// * `install_dir` - `ClipRefiner.exe` のあるフォルダ
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    pub(crate) fn new(install_dir: &Path) -> Self {
        Self {
            exe: install_dir.join(portable_exe_name()),
            install_dir: install_dir.to_path_buf(),
            started: None,
            child: None,
        }
    }

    /// 常駐アプリを起動し、常駐が完了するまで待つ
    ///
    /// 前回の呼び出しで起動したプロセスが残っていれば、起動し直さずに完了を待つ
    ///
    /// # Returns
    /// * `Result<(), UpdateError>` - 常駐が完了した場合は `Ok(())`
    pub(crate) fn start(&mut self) -> Result<(), UpdateError> {
        self.start_within(START_TIMEOUT)
    }

    /// 待機の上限を指定して起動する
    fn start_within(&mut self, timeout: Duration) -> Result<(), UpdateError> {
        // 起動完了の通知を取りこぼさないよう、プロセスより先に作る
        let event = match self.started.take() {
            Some(event) => event,
            None => NamedEvent::create(&startup_complete_event_name(), true).map_err(|err| {
                tracing::warn!("起動完了イベントの作成に失敗: {err}");
                UpdateError::StartFailed
            })?,
        };
        let event = self.started.insert(event);

        let alive = self
            .child
            .take()
            .and_then(|mut child| matches!(child.try_wait(), Ok(None)).then_some(child));
        let child = if let Some(child) = alive {
            child
        } else {
            // 既に終了したプロセスの通知を、これから起動するプロセスのものと取り違えない
            event.reset().map_err(|err| {
                tracing::warn!("起動完了イベントの初期化に失敗: {err}");
                UpdateError::StartFailed
            })?;
            Command::new(&self.exe)
                .current_dir(&self.install_dir)
                .spawn()
                .map_err(|err| {
                    tracing::warn!("常駐アプリの起動に失敗: {err}");
                    UpdateError::StartFailed
                })?
        };
        let child = self.child.insert(child);

        match event.wait_with_process(child.as_handle(), Some(timeout)) {
            // 通知の直後に異常終了したものは、動作しているとみなさない
            Ok(EventWait::Signaled) if matches!(child.try_wait(), Ok(None)) => Ok(()),
            Ok(EventWait::Signaled | EventWait::ProcessExited) => {
                tracing::warn!("常駐アプリが起動の途中で終了した: {:?}", child.try_wait());
                Err(UpdateError::StartExited)
            }
            Ok(EventWait::TimedOut) => {
                tracing::warn!("常駐アプリの起動完了を確認できない");
                Err(UpdateError::StartTimeout)
            }
            Err(err) => {
                tracing::warn!("常駐アプリの起動待ちに失敗: {err}");
                Err(UpdateError::StartTimeout)
            }
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::w;

    use super::*;

    /// PID `0` と自身の PID を拒否すること
    #[test]
    fn open_rejects_zero_and_self() {
        for pid in [0, std::process::id()] {
            assert_eq!(
                ResidentProcess::open(pid, Path::new("unused")).err(),
                Some(UpdateError::WaitPidInvalid)
            );
        }
    }

    /// 実行フォルダの `ClipRefiner.exe` ではないプロセスを拒否すること
    #[test]
    fn open_rejects_other_process() {
        let mut child = Command::new("ping")
            .args(["127.0.0.1", "-n", "6"])
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");

        let result = ResidentProcess::open(child.id(), &std::env::temp_dir());
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(result.err(), Some(UpdateError::WaitPidNotClipRefiner));
    }

    /// 既に終了している PID は、終了済みとして扱うこと
    #[test]
    fn missing_pid_is_treated_as_exited() {
        // 開発機で動作中の常駐アプリに左右されないよう、存在しない名前を使う
        let process = ResidentProcess::open_with_instance(
            0xFFFF_FFFC,
            Path::new("unused"),
            "clip-refiner-test-resident-none",
        )
        .expect("missing pid");
        assert_eq!(process.wait_exit(Duration::ZERO), Ok(true));
        assert_eq!(process.request_exit(), Ok(()));
    }

    /// 起動し直された別の常駐アプリがある間は、終了していないものとして扱うこと
    #[test]
    fn restarted_resident_is_not_treated_as_exited() {
        const NAME: &str = "clip-refiner-test-resident-restarted";
        // SAFETY: 引数は NUL 終端の静的文字列で、返るハンドルは直後に所有権を移す
        let mutex =
            unsafe { CreateMutexW(None, false, w!("clip-refiner-test-resident-restarted")) }
                .expect("mutex");
        // SAFETY: `CreateMutexW` が成功して返したハンドルで、ここで唯一の所有者になる
        let mutex = unsafe { OwnedHandle::from_raw_handle(mutex.0) };

        let process = ResidentProcess::open_with_instance(0xFFFF_FFFC, Path::new("unused"), NAME)
            .expect("missing pid");
        assert_eq!(process.wait_exit(Duration::ZERO), Ok(false));
        // 依頼先が分からないため、更新へ進めない
        assert_eq!(process.request_exit(), Err(UpdateError::ExitRequest));

        drop(mutex);
        assert_eq!(process.wait_exit(Duration::ZERO), Ok(true));
    }

    /// 実行ファイルが無いときは起動失敗として扱うこと
    #[test]
    fn start_fails_when_exe_is_missing() {
        let dir = std::env::temp_dir().join("clip-refiner-updater-missing-install-dir");
        let mut starter = ResidentStarter::new(&dir);
        assert_eq!(
            starter.start_within(Duration::from_secs(1)),
            Err(UpdateError::StartFailed)
        );
    }
}

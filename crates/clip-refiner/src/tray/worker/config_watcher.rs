//! 設定ディレクトリの `FindFirstChangeNotification` 監視
//!
//! 本文は読まない。起床だけをワーカーへ伝え、変更判定は内容指紋に任せる

use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use thiserror::Error;
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_CREATION, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
    FILE_NOTIFY_CHANGE_SIZE, FindCloseChangeNotification, FindFirstChangeNotificationW,
    FindNextChangeNotification,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::PCWSTR;

use super::command::ClipboardCommand;

// ======================================================================
// 定数
// ======================================================================
/// 停止フラグ確認のための待ち上限
const WATCH_WAIT_MS: u32 = 500;

// ======================================================================
// エラー
// ======================================================================
/// 設定ディレクトリ監視の起動失敗
#[derive(Debug, Error)]
pub(super) enum ConfigWatcherError {
    /// 専用スレッドの spawn 失敗
    #[error("設定監視スレッドの起動に失敗: {0}")]
    Spawn(String),
    /// 監視対象ディレクトリの解決または通知ハンドルの初期化失敗
    #[error("設定ディレクトリ監視の初期化に失敗: {0}")]
    Init(String),
    /// 準備完了通知前にスレッドが終了した
    #[error("設定監視スレッドが準備完了前に終了した")]
    ReadyChannelClosed,
}

// ======================================================================
// 連打抑制
// ======================================================================
/// 未処理の変更通知が無ければ `ConfigFileChanged` を送る
///
/// # Arguments
/// * `pending` - 未処理の `ConfigFileChanged` があるかを示すフラグ
/// * `tx` - ワーカーへのコマンド送信チャネル
///
/// # Returns
/// * `bool` - 新たにコマンドを送った場合は `true`
pub(crate) fn try_notify_config_changed(
    pending: &AtomicBool,
    tx: &Sender<ClipboardCommand>,
) -> bool {
    if pending.swap(true, Ordering::AcqRel) {
        return false;
    }

    if tx.send(ClipboardCommand::ConfigFileChanged).is_err() {
        pending.store(false, Ordering::Release);
        return false;
    }
    true
}

/// 未処理フラグを下げ、次の変更通知を受けられるようにする
///
/// # Arguments
/// * `pending` - 未処理の `ConfigFileChanged` があるかを示すフラグ
pub(crate) fn clear_config_changed_pending(pending: &AtomicBool) {
    pending.store(false, Ordering::Release);
}

// ======================================================================
// ウォッチャーハンドル
// ======================================================================
/// 設定ディレクトリ変更を専用スレッドで待ち、ワーカーへ起床通知する
pub(super) struct ConfigFileWatcher {
    stop: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl ConfigFileWatcher {
    /// 現在の設定ディレクトリを監視するスレッドを起動する
    ///
    /// # Arguments
    /// * `tx` - ワーカーへのコマンド送信チャネル
    /// * `pending` - `ConfigFileChanged` の連打抑制フラグ
    ///
    /// # Returns
    /// * `Result<Self, ConfigWatcherError>` - 起動成功時はハンドル
    pub fn start(
        tx: Sender<ClipboardCommand>,
        pending: Arc<AtomicBool>,
    ) -> Result<Self, ConfigWatcherError> {
        let watch_dir = crate::config::get_config_dir()
            .map_err(|err| ConfigWatcherError::Init(err.to_string()))?;
        Self::start_watching(watch_dir, tx, pending)
    }

    /// 指定ディレクトリを監視するスレッドを起動する
    ///
    /// 呼び出しスレッドで解決したパスを渡す (テスト用設定ディレクトリは thread-local のため)
    ///
    /// # Arguments
    /// * `watch_dir` - 監視する設定ディレクトリ
    /// * `tx` - ワーカーへのコマンド送信チャネル
    /// * `pending` - `ConfigFileChanged` の連打抑制フラグ
    ///
    /// # Returns
    /// * `Result<Self, ConfigWatcherError>` - 起動成功時はハンドル
    pub(crate) fn start_watching(
        watch_dir: PathBuf,
        tx: Sender<ClipboardCommand>,
        pending: Arc<AtomicBool>,
    ) -> Result<Self, ConfigWatcherError> {
        fs::create_dir_all(&watch_dir).map_err(|err| ConfigWatcherError::Init(err.to_string()))?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("clip-refiner-config-watcher".into())
            .spawn(move || run_watch_loop(&watch_dir, &tx, &pending, &stop_flag, &ready_tx))
            .map_err(|err| ConfigWatcherError::Spawn(err.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                join: Mutex::new(Some(join)),
            }),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(ConfigWatcherError::ReadyChannelClosed),
        }
    }

    /// 監視を停止し、スレッド終了を待つ
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.join.lock()
            && let Some(join) = guard.take()
        {
            let _ = join.join();
        }
    }
}

impl Drop for ConfigFileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// ディレクトリ変更を待ち、ワーカーへ通知する
fn run_watch_loop(
    watch_dir: &Path,
    tx: &Sender<ClipboardCommand>,
    pending: &Arc<AtomicBool>,
    stop: &AtomicBool,
    ready_tx: &Sender<Result<(), ConfigWatcherError>>,
) {
    let wide = path_to_wide(watch_dir);
    let notify_filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_CREATION;
    // SAFETY: `wide` は NUL 終端の UTF-16 で、呼び出し中は生存する
    let change = match unsafe {
        FindFirstChangeNotificationW(PCWSTR(wide.as_ptr()), false, notify_filter)
    } {
        Ok(handle) => handle,
        Err(err) => {
            let _ = ready_tx.send(Err(ConfigWatcherError::Init(err.to_string())));
            return;
        }
    };

    if ready_tx.send(Ok(())).is_err() {
        close_change_notification(change);
        return;
    }

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // SAFETY: `change` はループ終了後まで閉じない生存中の変更通知ハンドル
        let wait = unsafe { WaitForSingleObject(change, WATCH_WAIT_MS) };
        if wait == WAIT_OBJECT_0 {
            let _ = try_notify_config_changed(pending, tx);
            // SAFETY: `change` は生存中の変更通知ハンドル
            if let Err(err) = unsafe { FindNextChangeNotification(change) } {
                crate::log_warn!("設定ディレクトリ監視の継続に失敗: {err}");
                break;
            }
        } else if wait != WAIT_TIMEOUT {
            crate::log_warn!("設定ディレクトリ監視が予期しない待ち結果で終了");
            break;
        }
    }

    close_change_notification(change);
}

fn close_change_notification(handle: HANDLE) {
    // SAFETY: 呼び出し元は `FindFirstChangeNotificationW` で得たハンドルを 1 度だけ渡し、以降は使わない
    if let Err(err) = unsafe { FindCloseChangeNotification(handle) } {
        crate::log_warn!("設定ディレクトリ監視ハンドルの解放に失敗: {err}");
    }
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::config::AppConfig;
    use crate::test_helpers::with_temp_config_dir;

    /// 未処理が無いときは `ConfigFileChanged` を送るようにすること
    #[test]
    fn try_notify_sends_when_not_pending() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_config_changed(&pending, &tx));
        assert!(pending.load(Ordering::Acquire));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ConfigFileChanged)
        ));
    }

    /// 未処理中は追加の `ConfigFileChanged` を送らないようにすること
    #[test]
    fn try_notify_coalesces_while_pending() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_config_changed(&pending, &tx));
        assert!(!try_notify_config_changed(&pending, &tx));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ConfigFileChanged)
        ));
        assert!(rx.try_recv().is_err());
    }

    /// フラグを下げた後は再び通知できるようにすること
    #[test]
    fn clear_pending_allows_next_notify() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_config_changed(&pending, &tx));
        let _ = rx.try_recv();
        clear_config_changed_pending(&pending);

        assert!(try_notify_config_changed(&pending, &tx));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ConfigFileChanged)
        ));
    }

    /// 送信失敗時は未処理フラグを戻すようにすること
    #[test]
    fn try_notify_clears_pending_when_send_fails() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let pending = AtomicBool::new(false);

        assert!(!try_notify_config_changed(&pending, &tx));
        assert!(!pending.load(Ordering::Acquire));
    }

    /// 起動失敗の `Display` がユーザー向け文言になるようにすること
    #[test]
    fn config_watcher_error_display() {
        let spawn = ConfigWatcherError::Spawn("os error".into());
        assert!(spawn.to_string().contains("設定監視スレッドの起動に失敗"));

        let init = ConfigWatcherError::Init("watch".into());
        assert!(
            init.to_string()
                .contains("設定ディレクトリ監視の初期化に失敗")
        );

        assert_eq!(
            ConfigWatcherError::ReadyChannelClosed.to_string(),
            "設定監視スレッドが準備完了前に終了した"
        );
    }

    /// 設定ファイルの書き込みで `ConfigFileChanged` を受け取れるようにすること
    #[test]
    fn watcher_notifies_on_config_write() {
        with_temp_config_dir(|| {
            let watch_dir = crate::config::get_config_dir().expect("config dir");
            let (tx, rx) = mpsc::channel();
            let pending = Arc::new(AtomicBool::new(false));
            let watcher =
                ConfigFileWatcher::start_watching(watch_dir, tx, pending).expect("watcher start");

            AppConfig::default().save().expect("config save");

            let received = rx.recv_timeout(Duration::from_secs(3));
            watcher.stop();
            assert!(
                matches!(received, Ok(ClipboardCommand::ConfigFileChanged)),
                "書き込み後に ConfigFileChanged を受け取ること: {received:?}"
            );
        });
    }
}

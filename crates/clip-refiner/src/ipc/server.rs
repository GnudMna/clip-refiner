//! ワンショット IPC サーバー (常駐側)
//!
//! パイプは現在ユーザー専用 DACL で作成し、他ユーザーからの接続を拒否する

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::protocol::{
    MAX_IPC_MESSAGE_BYTES, OneshotIpcRequest, OneshotIpcResponse, parse_request,
};
use crate::config::permissions::OwnerOnlySecurityAttributes;
use crate::consts;
use crate::tray::worker::ClipboardWorkerHandle;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::core::PCWSTR;

// ======================================================================
// 定数
// ======================================================================
/// ワーカー応答待ちの上限
const WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// パイプ入出力バッファサイズ
const PIPE_BUFFER_SIZE: u32 = 64 * 1024;

// ======================================================================
// サーバーハンドル
// ======================================================================
/// ワンショット IPC サーバーのライフタイム管理
pub struct OneshotIpcServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pipe_name: String,
}

impl OneshotIpcServer {
    /// 本番パイプ名で待受スレッドを起動する
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    ///
    /// # Returns
    /// * `Self` - 停止時に待受を終了するガード
    pub fn start(worker: Arc<ClipboardWorkerHandle>) -> Self {
        Self::start_named(worker, consts::ONESHOT_IPC_PIPE_NAME.to_owned())
    }

    /// 指定パイプ名で待受スレッドを起動する
    ///
    /// 統合テストでは本番パイプと衝突しない一意な名前を渡す
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    /// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
    ///
    /// # Returns
    /// * `Self` - 停止時に待受を終了するガード
    pub(crate) fn start_named(worker: Arc<ClipboardWorkerHandle>, pipe_name: String) -> Self {
        // テスト用パイプは FIRST_PIPE_INSTANCE を付けない。
        // 連続接続でインスタンス再作成が失敗し、クライアントが応答待ちのまま残るのを避ける
        let first_instance_only = pipe_name == consts::ONESHOT_IPC_PIPE_NAME;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let listen_name = pipe_name.clone();
        let thread = thread::Builder::new()
            .name("clip-refiner-oneshot-ipc".into())
            .spawn(move || {
                run_server_loop(&worker, &stop_flag, &listen_name, first_instance_only);
            })
            .ok();

        Self {
            stop,
            thread,
            pipe_name,
        }
    }

    /// 待受を停止し、ブロッキング中の `ConnectNamedPipe` を解除する
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // 待受解除用のダミー接続
        let _ = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.pipe_name);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for OneshotIpcServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ======================================================================
// 待受ループ
// ======================================================================
fn run_server_loop(
    worker: &ClipboardWorkerHandle,
    stop: &AtomicBool,
    pipe_name: &str,
    first_instance_only: bool,
) {
    while !stop.load(Ordering::SeqCst) {
        let pipe = match create_pipe_instance_named(pipe_name, first_instance_only) {
            Ok(pipe) => pipe,
            Err(err) => {
                crate::log_warn!("ワンショット IPC パイプの作成に失敗: {err}");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
        };

        match connect_pipe(&pipe) {
            Ok(()) => {}
            Err(err) => {
                crate::log_debug!("ワンショット IPC 接続待ち失敗: {err}");
                continue;
            }
        }

        if stop.load(Ordering::SeqCst) {
            break;
        }

        handle_connected_client(pipe, worker);
    }
}

/// 指定名で現在ユーザー専用 DACL 付きパイプを作成する
///
/// # Arguments
/// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
/// * `first_instance_only` - `true` のとき `FILE_FLAG_FIRST_PIPE_INSTANCE` を付ける
///
/// # Returns
/// * `Result<File>` - 作成したパイプハンドル
///
/// # Errors
/// セキュリティ記述子の構築または `CreateNamedPipeW` に失敗した場合
fn create_pipe_instance_named(pipe_name: &str, first_instance_only: bool) -> Result<File> {
    let security = OwnerOnlySecurityAttributes::new()
        .context("ワンショット IPC のセキュリティ記述子の構築に失敗")?;

    let mut name: Vec<u16> = pipe_name.encode_utf16().collect();
    name.push(0);

    let mut open_mode = PIPE_ACCESS_DUPLEX;
    if first_instance_only {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            0,
            Some(security.as_ptr()),
        )
    };

    if handle.is_invalid() {
        return Err(anyhow::anyhow!(
            "名前付きパイプの作成に失敗: {}",
            windows::core::Error::from_thread()
        ));
    }

    Ok(unsafe { File::from_raw_handle(handle.0 as RawHandle) })
}

fn connect_pipe(pipe: &File) -> windows::core::Result<()> {
    let handle = HANDLE(pipe.as_raw_handle());
    unsafe {
        match ConnectNamedPipe(handle, None) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == ERROR_PIPE_CONNECTED.into() => Ok(()),
            Err(err) => Err(err),
        }
    }
}

fn handle_connected_client(pipe: File, worker: &ClipboardWorkerHandle) {
    let mut reader = BufReader::new(pipe);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => return,
        Ok(_) => {}
        Err(err) => {
            crate::log_warn!("ワンショット IPC 要求の読み取りに失敗: {err}");
            return;
        }
    }

    if line.len() > MAX_IPC_MESSAGE_BYTES {
        crate::log_warn!("ワンショット IPC 要求が大きすぎるため破棄");
        return;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    let request = match parse_request(trimmed) {
        Ok(request) => request,
        Err(err) => {
            crate::log_warn!("ワンショット IPC 要求の解析に失敗: {err}");
            let _ = write_response(
                reader.get_mut(),
                &OneshotIpcResponse::error(String::new(), "要求を解釈できませんでした"),
            );
            return;
        }
    };

    let response = dispatch_request(request, worker);
    if let Err(err) = write_response(reader.get_mut(), &response) {
        crate::log_warn!("ワンショット IPC 応答の書き込みに失敗: {err}");
    }
}

fn dispatch_request(
    request: OneshotIpcRequest,
    worker: &ClipboardWorkerHandle,
) -> OneshotIpcResponse {
    let request_id = request.request_id().to_string();
    let (pipeline, refine_ctx) = request.into_pipeline_and_context();

    if pipeline.is_empty() {
        return OneshotIpcResponse::error(request_id, "加工パイプラインが空です");
    }

    match worker.process_oneshot(pipeline, refine_ctx, WORKER_REPLY_TIMEOUT) {
        Ok(result) => result.into_response(request_id),
        Err(err) => OneshotIpcResponse::error(request_id, err.to_string()),
    }
}

fn write_response(stream: &mut File, response: &OneshotIpcResponse) -> std::io::Result<()> {
    let body = serde_json::to_string(response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(stream, "{body}")?;
    stream.flush()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{BufRead, BufReader, Write};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use super::{connect_pipe, create_pipe_instance_named};

    /// 現在ユーザー専用 DACL のパイプへ同一ユーザーが接続できること
    #[test]
    fn owner_only_pipe_allows_same_user_connect() {
        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-acl-test-{}",
            std::process::id()
        );
        let barrier = Arc::new(Barrier::new(2));
        let server_barrier = Arc::clone(&barrier);
        let server_name = pipe_name.clone();

        let server = thread::spawn(move || {
            let pipe = create_pipe_instance_named(&server_name, true).expect("create pipe");
            server_barrier.wait();
            connect_pipe(&pipe).expect("connect pipe");
            let mut reader = BufReader::new(pipe);
            let mut line = String::new();
            reader.read_line(&mut line).expect("read");
            assert_eq!(line.trim(), "ping");
            let mut pipe = reader.into_inner();
            writeln!(pipe, "pong").expect("write");
            pipe.flush().expect("flush");
        });

        barrier.wait();
        // パイプ作成直後の接続競合を避ける
        thread::sleep(Duration::from_millis(20));

        let mut client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .expect("open pipe as current user");
        writeln!(client, "ping").expect("client write");
        client.flush().expect("client flush");

        let mut reader = BufReader::new(client);
        let mut line = String::new();
        reader.read_line(&mut line).expect("client read");
        assert_eq!(line.trim(), "pong");

        server.join().expect("server join");
    }
}

//! ワンショット IPC サーバー (常駐側)

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

use crate::consts;
use crate::tray::worker::ClipboardWorkerHandle;

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
}

impl OneshotIpcServer {
    /// 名前付きパイプの待受スレッドを起動する
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    ///
    /// # Returns
    /// * `Self` - 停止時に待受を終了するガード
    pub fn start(worker: Arc<ClipboardWorkerHandle>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("clip-refiner-oneshot-ipc".into())
            .spawn(move || run_server_loop(&worker, &stop_flag))
            .ok();

        Self { stop, thread }
    }

    /// 待受を停止し、ブロッキング中の `ConnectNamedPipe` を解除する
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // 待受解除用のダミー接続
        let _ = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(consts::ONESHOT_IPC_PIPE_NAME);
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
fn run_server_loop(worker: &ClipboardWorkerHandle, stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        let pipe = match create_pipe_instance() {
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

fn create_pipe_instance() -> windows::core::Result<File> {
    let mut name: Vec<u16> = consts::ONESHOT_IPC_PIPE_NAME.encode_utf16().collect();
    name.push(0);

    let open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE;
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            0,
            None,
        )
    };

    if handle.is_invalid() {
        return Err(windows::core::Error::from_thread());
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

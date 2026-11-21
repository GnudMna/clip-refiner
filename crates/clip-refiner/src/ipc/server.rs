//! ワンショット IPC サーバー (常駐側)
//!
//! パイプは現在ユーザー専用 DACL で作成し、他ユーザーからの接続を拒否する
//! 接続ごとに処理スレッドへ渡し、応答待ちの接続があっても次の接続を受け付ける
//! 要求の読み取りには期限を設け、要求を送らない接続が処理枠を占有し続けないようにする

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::protocol::{
    BoundedLine, OneshotIpcRequest, OneshotIpcResponse, parse_request, read_bounded_line,
};
use super::session::resident_pipe_name;
use crate::config::permissions::OwnerOnlySecurityAttributes;
use crate::tray::worker::ClipboardWorkerHandle;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT, PeekNamedPipe,
};
use windows::core::PCWSTR;

// ======================================================================
// 定数
// ======================================================================
/// ワーカー応答待ちの上限
const WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// 接続から要求 1 行を受け取り終えるまでの上限
pub(crate) const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// 要求の到着を確認する間隔
const READ_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// パイプ入出力バッファサイズ
const PIPE_BUFFER_SIZE: u32 = 64 * 1024;
/// 同時に処理する接続数の上限
const MAX_ACTIVE_CONNECTIONS: u32 = 4;
/// パイプインスタンス数の上限 (処理中の接続 + 待受 1)
const MAX_PIPE_INSTANCES: u32 = MAX_ACTIVE_CONNECTIONS + 1;
/// 処理枠の空きを確認する間隔
const SLOT_WAIT_INTERVAL: Duration = Duration::from_millis(20);

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
    /// 現在のログオンセッション用の本番パイプ名で待受スレッドを起動する
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    ///
    /// # Returns
    /// * `Result<Self>` - 停止時に待受を終了するガード
    ///
    /// # Errors
    /// パイプ名の解決または待受スレッドの起動に失敗した場合
    pub fn start(worker: Arc<ClipboardWorkerHandle>) -> Result<Self> {
        let pipe_name = resident_pipe_name().context("ワンショット IPC のパイプ名の解決に失敗")?;
        // 他プロセスによる本番パイプ名の先取りを避けるため FIRST_PIPE_INSTANCE を付ける
        Self::spawn(worker, pipe_name, REQUEST_READ_TIMEOUT, true)
    }

    /// 指定パイプ名で待受スレッドを起動する
    ///
    /// 統合テストでは本番パイプと衝突しない一意な名前と、短い読み取り期限を渡す
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    /// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
    /// * `read_timeout` - 接続から要求 1 行を受け取り終えるまでの上限
    ///
    /// # Returns
    /// * `Result<Self>` - 停止時に待受を終了するガード
    ///
    /// # Errors
    /// 待受スレッドの起動に失敗した場合
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub(crate) fn start_named(
        worker: Arc<ClipboardWorkerHandle>,
        pipe_name: String,
        read_timeout: Duration,
    ) -> Result<Self> {
        // テスト用パイプは FIRST_PIPE_INSTANCE を付けない
        // 連続接続でインスタンス再作成が失敗し、クライアントが応答待ちのまま残るのを避ける
        Self::spawn(worker, pipe_name, read_timeout, false)
    }

    /// 待受スレッドを起動する
    ///
    /// # Arguments
    /// * `worker` - クリップボードワーカーへのハンドル
    /// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
    /// * `read_timeout` - 接続から要求 1 行を受け取り終えるまでの上限
    /// * `first_instance_only` - 手持ちインスタンスが無いときに `FILE_FLAG_FIRST_PIPE_INSTANCE` を付けるか
    ///
    /// # Returns
    /// * `Result<Self>` - 停止時に待受を終了するガード
    ///
    /// # Errors
    /// 待受スレッドの起動に失敗した場合
    fn spawn(
        worker: Arc<ClipboardWorkerHandle>,
        pipe_name: String,
        read_timeout: Duration,
        first_instance_only: bool,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let listen_name = pipe_name.clone();
        let thread = thread::Builder::new()
            .name("clip-refiner-oneshot-ipc".into())
            .spawn(move || {
                run_server_loop(
                    &worker,
                    &stop_flag,
                    &listen_name,
                    first_instance_only,
                    read_timeout,
                );
            })
            .context("ワンショット IPC 待受スレッドの起動に失敗")?;

        Ok(Self {
            stop,
            thread: Some(thread),
            pipe_name,
        })
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
// 処理中の接続
// ======================================================================
/// 処理スレッドへ渡した接続
///
/// 破棄時に処理中の接続数を減らし、待受ループの処理枠を空ける
struct ActiveConnection {
    /// 接続済みのパイプ
    pipe: File,
    /// 処理中の接続数
    active: Arc<AtomicU32>,
}

impl ActiveConnection {
    /// 処理中の接続数を増やして接続を登録する
    ///
    /// # Arguments
    /// * `pipe` - 接続済みのパイプ
    /// * `active` - 処理中の接続数
    ///
    /// # Returns
    /// * `Self` - 破棄時に接続数を戻すガード
    fn register(pipe: File, active: &Arc<AtomicU32>) -> Self {
        active.fetch_add(1, Ordering::SeqCst);
        Self {
            pipe,
            active: Arc::clone(active),
        }
    }
}

impl Drop for ActiveConnection {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

// ======================================================================
// 期限付き読み取り
// ======================================================================
/// 期限までに届いた分だけを読み取るパイプリーダー
///
/// 同期パイプの `ReadFile` は届くまで戻らないため、`PeekNamedPipe` で到着済みの
/// バイト数を確かめ、その範囲だけを読む。期限を過ぎると `TimedOut` を返す
struct DeadlineReader<'a> {
    /// 接続済みのパイプ
    pipe: &'a File,
    /// 読み取りを打ち切る時刻
    deadline: Instant,
}

impl<'a> DeadlineReader<'a> {
    /// 現在時刻から `timeout` 後を期限とするリーダーを作る
    ///
    /// # Arguments
    /// * `pipe` - 接続済みのパイプ
    /// * `timeout` - 読み取り全体の上限
    ///
    /// # Returns
    /// * `Self` - 期限付きリーダー
    fn new(pipe: &'a File, timeout: Duration) -> Self {
        Self {
            pipe,
            deadline: Instant::now() + timeout,
        }
    }
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            // クライアント切断後も未読データは読めるため、到着済みの分を先に返す
            let Some(available) = pipe_available_bytes(self.pipe)? else {
                return Ok(0);
            };
            if available > 0 {
                let len = buf.len().min(available as usize);
                let mut pipe = self.pipe;
                return pipe.read(&mut buf[..len]);
            }
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "要求の受信が期限を超えた",
                ));
            }
            thread::sleep(READ_POLL_INTERVAL);
        }
    }
}

/// パイプに到着済みで未読のバイト数を返す
///
/// # Arguments
/// * `pipe` - 接続済みのパイプ
///
/// # Returns
/// * `io::Result<Option<u32>>` - 未読バイト数。クライアントが切断し未読データも無い場合は `None`
fn pipe_available_bytes(pipe: &File) -> io::Result<Option<u32>> {
    let mut available = 0u32;
    // SAFETY: `pipe` が所有する有効なパイプハンドルを借用し、`available` は呼び出し中に生存する
    let peeked = unsafe {
        PeekNamedPipe(
            HANDLE(pipe.as_raw_handle()),
            None,
            0,
            None,
            Some(&raw mut available),
            None,
        )
    };
    match peeked {
        Ok(()) => Ok(Some(available)),
        Err(err) if err.code() == ERROR_BROKEN_PIPE.into() => Ok(None),
        Err(err) => Err(io::Error::other(err)),
    }
}

// ======================================================================
// 待受ループ
// ======================================================================
fn run_server_loop(
    worker: &Arc<ClipboardWorkerHandle>,
    stop: &AtomicBool,
    pipe_name: &str,
    first_instance_only: bool,
    read_timeout: Duration,
) {
    let active = Arc::new(AtomicU32::new(0));
    let mut listening: Option<File> = None;

    while !stop.load(Ordering::SeqCst) {
        // 手持ちのインスタンスが無いときは他プロセスの先取りを避けるため FIRST_PIPE_INSTANCE で作る
        // (処理中の接続が残っている間は作成に失敗し、再試行する)
        let pipe = match listening.take() {
            Some(pipe) => pipe,
            None => match create_pipe_instance_named(pipe_name, first_instance_only) {
                Ok(pipe) => pipe,
                Err(err) => {
                    crate::log_warn!("ワンショット IPC パイプの作成に失敗: {err}");
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }
            },
        };

        // 作成前に停止要求とダミー接続が済んでいた場合に `ConnectNamedPipe` で止まらないよう確認する
        if stop.load(Ordering::SeqCst) {
            break;
        }

        if let Err(err) = connect_pipe(&pipe) {
            crate::log_debug!("ワンショット IPC 接続待ち失敗: {err}");
            continue;
        }

        if stop.load(Ordering::SeqCst) || !wait_for_free_slot(&active, stop) {
            break;
        }

        let connection = ActiveConnection::register(pipe, &active);
        // 接続を保持している間に次の待受インスタンスを作る (FIRST_PIPE_INSTANCE は付けない)
        listening = match create_pipe_instance_named(pipe_name, false) {
            Ok(next) => Some(next),
            Err(err) => {
                crate::log_debug!("ワンショット IPC 次インスタンスの作成に失敗: {err}");
                None
            }
        };
        spawn_connection_handler(connection, worker, read_timeout);
    }
}

/// 処理中の接続数が上限未満になるまで待つ
///
/// # Arguments
/// * `active` - 処理中の接続数
/// * `stop` - 停止要求フラグ
///
/// # Returns
/// * `bool` - 処理枠が空いた場合は `true`、待機中に停止要求を受けた場合は `false`
fn wait_for_free_slot(active: &AtomicU32, stop: &AtomicBool) -> bool {
    while active.load(Ordering::SeqCst) >= MAX_ACTIVE_CONNECTIONS {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        thread::sleep(SLOT_WAIT_INTERVAL);
    }
    true
}

/// 接続済みクライアントを専用スレッドで処理する
///
/// スレッドを起動できない場合は接続を閉じる (クライアントは切断を検知する)
///
/// # Arguments
/// * `connection` - 処理中として登録済みの接続
/// * `worker` - クリップボードワーカーへのハンドル
/// * `read_timeout` - 要求 1 行を受け取り終えるまでの上限
fn spawn_connection_handler(
    connection: ActiveConnection,
    worker: &Arc<ClipboardWorkerHandle>,
    read_timeout: Duration,
) {
    let worker = Arc::clone(worker);
    let spawned = thread::Builder::new()
        .name("clip-refiner-oneshot-ipc-conn".into())
        .spawn(move || {
            handle_connected_client(&connection.pipe, &worker, read_timeout);
        });
    if let Err(err) = spawned {
        crate::log_warn!("ワンショット IPC 接続処理スレッドの起動に失敗: {err}");
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
pub(super) fn create_pipe_instance_named(
    pipe_name: &str,
    first_instance_only: bool,
) -> Result<File> {
    let security = OwnerOnlySecurityAttributes::new()
        .context("ワンショット IPC のセキュリティ記述子の構築に失敗")?;

    let mut name: Vec<u16> = pipe_name.encode_utf16().collect();
    name.push(0);

    let mut open_mode = PIPE_ACCESS_DUPLEX;
    if first_instance_only {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    // SAFETY: `name` は NUL 終端の UTF-16 で、`security` は呼び出し完了まで生存する
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            MAX_PIPE_INSTANCES,
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

    // SAFETY: `handle` は直前に作成した有効なハンドルで、所有権を `File` へ移し他では閉じない
    Ok(unsafe { File::from_raw_handle(handle.0 as RawHandle) })
}

fn connect_pipe(pipe: &File) -> windows::core::Result<()> {
    let handle = HANDLE(pipe.as_raw_handle());
    // SAFETY: `pipe` が所有する有効なハンドルを借用する
    // パイプは `FILE_FLAG_OVERLAPPED` なしで作成しているため、`OVERLAPPED` なしの同期呼び出しでよい
    unsafe {
        match ConnectNamedPipe(handle, None) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == ERROR_PIPE_CONNECTED.into() => Ok(()),
            Err(err) => Err(err),
        }
    }
}

fn handle_connected_client(pipe: &File, worker: &ClipboardWorkerHandle, read_timeout: Duration) {
    let mut reader = BufReader::new(DeadlineReader::new(pipe, read_timeout));
    let line = match read_bounded_line(&mut reader) {
        Ok(BoundedLine::Line(line)) => line,
        Ok(BoundedLine::Eof) => return,
        Ok(BoundedLine::TooLarge) => {
            crate::log_warn!("ワンショット IPC 要求が大きすぎるため破棄");
            return;
        }
        Err(err) if err.kind() == io::ErrorKind::TimedOut => {
            crate::log_warn!("ワンショット IPC 要求が期限内に届かないため切断");
            return;
        }
        Err(err) => {
            crate::log_warn!("ワンショット IPC 要求の読み取りに失敗: {err}");
            return;
        }
    };

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    let request = match parse_request(trimmed) {
        Ok(request) => request,
        Err(err) => {
            crate::log_warn!("ワンショット IPC 要求の解析に失敗: {err}");
            let _ = write_response(
                pipe,
                &OneshotIpcResponse::error(String::new(), "要求を解釈できませんでした"),
            );
            return;
        }
    };

    let response = dispatch_request(request, worker);
    if let Err(err) = write_response(pipe, &response) {
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

fn write_response(mut stream: &File, response: &OneshotIpcResponse) -> std::io::Result<()> {
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
    use std::fs::{File, OpenOptions};
    use std::io::{self, BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::super::protocol::{BoundedLine, read_bounded_line};
    use super::{
        DeadlineReader, MAX_ACTIVE_CONNECTIONS, OneshotIpcServer, REQUEST_READ_TIMEOUT,
        connect_pipe, create_pipe_instance_named, wait_for_free_slot,
    };
    use crate::test_helpers::{stub_worker, test_app_state_arc, wait_for_oneshot_pipe};

    /// 期限付き読み取りテストの期限
    const SHORT_READ_TIMEOUT: Duration = Duration::from_millis(200);

    /// 接続済みのサーバー側パイプとクライアントを作る
    fn connected_pair(label: &str) -> (File, File) {
        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-deadline-{label}-{}",
            std::process::id()
        );
        let server = create_pipe_instance_named(&pipe_name, true).expect("create pipe");
        let client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .expect("open pipe");
        connect_pipe(&server).expect("connect pipe");
        (server, client)
    }

    /// 期限付きで 1 行読み取る
    fn read_line_with_deadline(server: &File) -> io::Result<BoundedLine> {
        let mut reader = BufReader::new(DeadlineReader::new(server, SHORT_READ_TIMEOUT));
        read_bounded_line(&mut reader)
    }

    /// 待受スレッド起動に成功したらサーバーガードを返すようにすること
    #[test]
    fn start_named_returns_ok_when_thread_spawns() {
        let (worker, _rx) = stub_worker(&test_app_state_arc());
        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-start-test-{}",
            std::process::id()
        );
        let server = OneshotIpcServer::start_named(worker, pipe_name.clone(), REQUEST_READ_TIMEOUT)
            .expect("待受スレッドを起動できる");
        assert!(
            wait_for_oneshot_pipe(&pipe_name, Duration::from_secs(2)),
            "待受開始を検知できること"
        );
        drop(server);
    }

    /// 現在ユーザー専用 DACL のパイプへ同一ユーザーが接続できるようにすること
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

    /// 処理枠に空きがあれば待たずに `true` を返すこと
    #[test]
    fn wait_for_free_slot_returns_true_when_below_limit() {
        let active = AtomicU32::new(MAX_ACTIVE_CONNECTIONS - 1);
        let stop = AtomicBool::new(false);
        assert!(wait_for_free_slot(&active, &stop));
    }

    /// 処理枠が埋まったまま停止要求を受けたら `false` を返すこと
    #[test]
    fn wait_for_free_slot_returns_false_on_stop() {
        let active = AtomicU32::new(MAX_ACTIVE_CONNECTIONS);
        let stop = AtomicBool::new(true);
        assert!(!wait_for_free_slot(&active, &stop));
    }

    /// 期限内に届いた要求行を読み取れるようにすること
    #[test]
    fn deadline_reader_reads_line_sent_in_time() {
        let (server, mut client) = connected_pair("line");
        writeln!(client, "ping").expect("client write");
        client.flush().expect("client flush");

        assert_eq!(
            read_line_with_deadline(&server).expect("read"),
            BoundedLine::Line("ping".into())
        );
    }

    /// 何も送らない接続は期限で `TimedOut` とすること
    #[test]
    fn deadline_reader_times_out_without_data() {
        let (server, _client) = connected_pair("idle");

        let started = Instant::now();
        let err = read_line_with_deadline(&server).expect_err("タイムアウトを期待");

        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        // 期限を過ぎてから打ち切り、無期限には待たない
        assert!(started.elapsed() >= SHORT_READ_TIMEOUT);
        assert!(started.elapsed() < SHORT_READ_TIMEOUT * 10);
    }

    /// 改行を送らないまま止まった接続も期限で `TimedOut` とすること
    #[test]
    fn deadline_reader_times_out_on_partial_line() {
        let (server, mut client) = connected_pair("partial");
        client.write_all(b"{\"type\":").expect("client write");
        client.flush().expect("client flush");

        let err = read_line_with_deadline(&server).expect_err("タイムアウトを期待");

        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    /// 何も送らずに切断した接続は EOF とすること
    #[test]
    fn deadline_reader_returns_eof_on_disconnect() {
        let (server, client) = connected_pair("disconnect");
        drop(client);

        assert_eq!(
            read_line_with_deadline(&server).expect("read"),
            BoundedLine::Eof
        );
    }

    /// 送信後に切断した接続でも未読の要求行を読み取れるようにすること
    #[test]
    fn deadline_reader_reads_pending_data_after_disconnect() {
        let (server, mut client) = connected_pair("pending");
        writeln!(client, "ping").expect("client write");
        client.flush().expect("client flush");
        drop(client);

        assert_eq!(
            read_line_with_deadline(&server).expect("read"),
            BoundedLine::Line("ping".into())
        );
    }
}

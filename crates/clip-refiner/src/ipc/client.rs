//! ワンショット IPC クライアント (CLI 側)

use std::fs::OpenOptions;
use std::io::{BufReader, Write};
use std::os::windows::io::AsRawHandle;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::protocol::{
    BoundedLine, MAX_IPC_MESSAGE_BYTES, OneshotIpcRequest, OneshotIpcResponse, build_request,
    parse_response, read_bounded_line,
};
use crate::consts;
use crate::refiner::{RefineContext, RefineMode};

use single_instance::SingleInstance;
use thiserror::Error;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::PeekNamedPipe;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
use windows::Win32::System::Pipes::WaitNamedPipeW;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
use windows::core::PCWSTR;

// ======================================================================
// 定数
// ======================================================================
/// 常駐パイプ接続のリトライ回数
const CONNECT_RETRY_COUNT: u32 = 8;
/// 接続リトライ間隔
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(75);
/// 応答読み取りの上限
const RESPONSE_READ_TIMEOUT: Duration = Duration::from_secs(30);

// ======================================================================
// エラー
// ======================================================================
/// ワンショット IPC クライアントの失敗理由
#[derive(Debug, Error)]
pub enum ClientError {
    /// 常駐プロセスのパイプに接続できない
    #[error("常駐プロセスへ接続できませんでした: {0}")]
    Connect(String),
    /// 要求の送受信に失敗した
    #[error("常駐プロセスとの通信に失敗しました: {0}")]
    Io(String),
    /// 応答 JSON が不正
    #[error("常駐プロセスからの応答を解釈できませんでした: {0}")]
    Protocol(String),
}

impl ClientError {
    /// 接続失敗としてリトライ可能かどうか
    fn is_retryable_connect(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
}

// ======================================================================
// パブリック関数
// ======================================================================
/// トレイ常駐インスタンスが起動中かどうか
///
/// 判定用に取得した `SingleInstance` は即座に破棄し、mutex を保持しない
///
/// # Returns
/// * `bool` - 常駐プロセスが既に存在する場合は `true`
pub fn is_resident_running() -> bool {
    match SingleInstance::new(consts::APP_ID) {
        Ok(instance) => !instance.is_single(),
        Err(_) => false,
    }
}

/// 常駐プロセスへワンショット加工を委譲する
///
/// # Arguments
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - CLI 側で解決済みの加工コンテキスト
///
/// # Returns
/// * `Result<OneshotIpcResponse, ClientError>` - 常駐側の応答
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
pub fn process_via_resident(
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<OneshotIpcResponse, ClientError> {
    process_via_pipe(consts::ONESHOT_IPC_PIPE_NAME, pipeline, ctx)
}

/// 指定パイプへワンショット加工を委譲する
///
/// # Arguments
/// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - CLI 側で解決済みの加工コンテキスト
///
/// # Returns
/// * `Result<OneshotIpcResponse, ClientError>` - 常駐側の応答
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
pub(crate) fn process_via_pipe(
    pipe_name: &str,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<OneshotIpcResponse, ClientError> {
    let request_id = new_request_id();
    let request = build_request(pipeline, ctx, request_id);
    let mut last_error = None;

    for attempt in 0..CONNECT_RETRY_COUNT {
        match send_request_to(pipe_name, &request) {
            Ok(response) => return Ok(response),
            Err(err) if err.is_retryable_connect() && attempt + 1 < CONNECT_RETRY_COUNT => {
                last_error = Some(err);
                thread::sleep(CONNECT_RETRY_INTERVAL);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Connect("不明なエラー".into())))
}

/// 指定パイプへ生の 1 行を送り、応答 JSON を待つ
///
/// 不正 JSON や上限超過など、プロトコル境界の検証用
///
/// # Arguments
/// * `pipe_name` - 接続先パイプ名
/// * `line` - 改行を含まない送信本文
///
/// # Returns
/// * `Result<OneshotIpcResponse, ClientError>` - 応答または通信エラー
///
/// # Errors
/// 接続・送受信・プロトコルエラーのいずれか
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn send_raw_line_to_pipe(
    pipe_name: &str,
    line: &str,
) -> Result<OneshotIpcResponse, ClientError> {
    let mut stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .map_err(|e| ClientError::Connect(e.to_string()))?;

    writeln!(stream, "{line}").map_err(|e| ClientError::Io(e.to_string()))?;
    stream.flush().map_err(|e| ClientError::Io(e.to_string()))?;
    read_response_line(&mut stream)
}

/// 名前付きパイプが接続可能になるまで待つ
///
/// パイプ未作成時の `WaitNamedPipeW` はタイムアウトを待たず即失敗するため、
/// 期限まで短い間隔で再試行する
///
/// # Arguments
/// * `pipe_name` - `\\.\pipe\...` 形式のパイプ名
/// * `timeout` - 待機上限
///
/// # Returns
/// * `bool` - タイムアウト前に待受を検知できた場合は `true`
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn wait_for_named_pipe(pipe_name: &str, timeout: Duration) -> bool {
    let mut name: Vec<u16> = pipe_name.encode_utf16().collect();
    name.push(0);
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // 未作成時は即失敗するため、1 回あたりの待ちは短くする
        // SAFETY: `name` は NUL 終端の UTF-16 で、呼び出し中は生存する
        if unsafe { WaitNamedPipeW(PCWSTR(name.as_ptr()), 50) }.as_bool() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

// ======================================================================
// プライベート関数
// ======================================================================
fn new_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{}", std::process::id(), nanos)
}

fn send_request_to(
    pipe_name: &str,
    request: &OneshotIpcRequest,
) -> Result<OneshotIpcResponse, ClientError> {
    let body = serde_json::to_string(request).map_err(|e| ClientError::Protocol(e.to_string()))?;
    if body.len() > MAX_IPC_MESSAGE_BYTES {
        return Err(ClientError::Protocol("要求が大きすぎます".into()));
    }

    let mut stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .map_err(|e| ClientError::Connect(e.to_string()))?;

    writeln!(stream, "{body}").map_err(|e| ClientError::Io(e.to_string()))?;
    stream.flush().map_err(|e| ClientError::Io(e.to_string()))?;
    read_response_line(&mut stream)
}

fn read_response_line(stream: &mut std::fs::File) -> Result<OneshotIpcResponse, ClientError> {
    let started = Instant::now();
    // サーバーが応答せず切断する経路 (過大要求破棄など) でも固まらないよう、
    // 受信データまたは切断をタイムアウト付きで待つ
    while !pipe_has_pending_data(stream) {
        if started.elapsed() > RESPONSE_READ_TIMEOUT {
            return Err(ClientError::Io("応答がタイムアウトしました".into()));
        }
        if !pipe_is_still_connected(stream) {
            return Err(ClientError::Io("常駐プロセスが応答前に切断しました".into()));
        }
        thread::sleep(Duration::from_millis(10));
    }

    let mut reader = BufReader::new(stream);
    loop {
        let line = match read_bounded_line(&mut reader) {
            Ok(BoundedLine::Line(line)) => line,
            Ok(BoundedLine::Eof) => {
                return Err(ClientError::Io("常駐プロセスが応答前に切断しました".into()));
            }
            Ok(BoundedLine::TooLarge) => {
                return Err(ClientError::Protocol("応答が大きすぎます".into()));
            }
            Err(e) => return Err(ClientError::Io(e.to_string())),
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started.elapsed() > RESPONSE_READ_TIMEOUT {
                return Err(ClientError::Io("応答がタイムアウトしました".into()));
            }
            continue;
        }
        return parse_response(trimmed).map_err(|e| ClientError::Protocol(e.to_string()));
    }
}

fn pipe_has_pending_data(stream: &std::fs::File) -> bool {
    let mut avail = 0u32;
    // SAFETY: `stream` が所有する有効なパイプハンドルを借用し、`avail` は呼び出し中に生存する
    let ok = unsafe {
        PeekNamedPipe(
            HANDLE(stream.as_raw_handle()),
            None,
            0,
            None,
            Some(&raw mut avail),
            None,
        )
    };
    ok.is_ok() && avail > 0
}

fn pipe_is_still_connected(stream: &std::fs::File) -> bool {
    let mut avail = 0u32;
    // SAFETY: `stream` が所有する有効なパイプハンドルを借用し、`avail` は呼び出し中に生存する
    unsafe {
        PeekNamedPipe(
            HANDLE(stream.as_raw_handle()),
            None,
            0,
            None,
            Some(&raw mut avail),
            None,
        )
    }
    .is_ok()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 存在しないパイプへの単発接続が失敗すること
    #[test]
    fn send_request_to_missing_pipe_returns_connect_error() {
        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-missing-{}",
            std::process::id()
        );
        let request = build_request(
            &[RefineMode::Trim],
            &RefineContext::default(),
            "req-missing",
        );
        let err = send_request_to(&pipe_name, &request).expect_err("接続失敗を期待");
        assert!(matches!(err, ClientError::Connect(_)));
    }

    /// シリアライズ後に上限を超える要求を接続前に拒否すること
    #[test]
    fn send_request_rejects_oversized_body_before_connect() {
        let mut ctx = RefineContext::default();
        ctx.regex = crate::config::RegexSettings::single("x".repeat(MAX_IPC_MESSAGE_BYTES), "");
        let request = build_request(&[RefineMode::RegexReplace], &ctx, "req-huge");
        let body = serde_json::to_string(&request).expect("serialize");
        assert!(body.len() > MAX_IPC_MESSAGE_BYTES);

        let pipe_name = format!(
            r"\\.\pipe\com.y_hirata.clip-refiner.oneshot-unused-{}",
            std::process::id()
        );
        let err = send_request_to(&pipe_name, &request).expect_err("サイズ超過を期待");
        assert!(matches!(err, ClientError::Protocol(_)));
    }
}
